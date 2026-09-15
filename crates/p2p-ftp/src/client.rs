//! 客户端：拨号控制流 + 数据流编排（FtpClient）。
//!
//! 一次传输 = 控制流收 `150 ok token=<hex>` → 开 `/ftp/data/1` 流出示
//! 令牌与操作码 → 原始字节定向流动 → 控制流收 `226`/错误码收尾。
//! RETR/STOR/APPE 为 64KiB 分片流式直通，不整文件驻内存。

use std::io;
use std::sync::Arc;

use p2p::Node;
use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::write_frame;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::vfs::parse_listing;
use crate::vfs::Entry;
use crate::wire::{
    data_header, Command, Reply, DATA_OP_GET, DATA_OP_LIST, DATA_OP_NLST, DATA_OP_PUT,
};
use crate::{proto, FtpError, PROTO_CTRL, PROTO_DATA};

/// FTP 客户端：单控制连接会话。并发传输不提供（与经典 FTP 控制连接同义）。
pub struct FtpClient {
    node: Arc<Node>,
    peer: PeerId,
    ctrl: BoxedStream,
}

impl FtpClient {
    /// 拨号并等待 220 问候（未登录，先 [FtpClient::login]）。
    pub async fn connect(node: Arc<Node>, peer: PeerId) -> Result<Self, FtpError> {
        node.connect(peer)
            .await
            .map_err(|e| FtpError::Assembly(e.to_string()))?;
        let mut ctrl = node
            .new_stream(peer, proto(PROTO_CTRL)?)
            .await
            .map_err(|e| FtpError::Assembly(e.to_string()))?;
        let greeting = Reply::read(&mut ctrl).await?;
        if greeting.code != 220 {
            return Err(FtpError::Rejected {
                code: greeting.code,
                text: greeting.text,
            });
        }
        Ok(Self { node, peer, ctrl })
    }

    /// USER/PASS 登录序列（服务端 331 → 230）。
    pub async fn login(&mut self, user: &str, pass: &str) -> Result<(), FtpError> {
        let r = self.cmd(&Command::User(user.to_string()).encode()).await?;
        self.require(&r, 331)?;
        let r = self.cmd(&Command::Pass(pass.to_string()).encode()).await?;
        self.require(&r, 230)
    }

    pub async fn pwd(&mut self) -> Result<String, FtpError> {
        let r = self.cmd("PWD").await?;
        self.require(&r, 257)?;
        r.text
            .split('"')
            .nth(1)
            .map(str::to_string)
            .ok_or_else(|| FtpError::BadReply(format!("unquoted path in 257: {}", r.text)))
    }

    pub async fn cwd(&mut self, path: &str) -> Result<(), FtpError> {
        let r = self.cmd(&format!("CWD {path}")).await?;
        self.require(&r, 250)
    }

    pub async fn mkd(&mut self, path: &str) -> Result<(), FtpError> {
        let r = self.cmd(&format!("MKD {path}")).await?;
        self.require(&r, 257)
    }

    pub async fn rmd(&mut self, path: &str) -> Result<(), FtpError> {
        let r = self.cmd(&format!("RMD {path}")).await?;
        self.require(&r, 250)
    }

    pub async fn dele(&mut self, path: &str) -> Result<(), FtpError> {
        let r = self.cmd(&format!("DELE {path}")).await?;
        self.require(&r, 250)
    }

    pub async fn rename(&mut self, from: &str, to: &str) -> Result<(), FtpError> {
        let r = self.cmd(&format!("RNFR {from}")).await?;
        self.require(&r, 350)?;
        let r = self.cmd(&format!("RNTO {to}")).await?;
        self.require(&r, 250)
    }

    pub async fn size(&mut self, path: &str) -> Result<u64, FtpError> {
        let r = self.cmd(&format!("SIZE {path}")).await?;
        self.require(&r, 213)?;
        r.text
            .trim()
            .parse()
            .map_err(|_| FtpError::BadReply(r.text.clone()))
    }

    pub async fn noop(&mut self) -> Result<(), FtpError> {
        let r = self.cmd("NOOP").await?;
        self.require(&r, 200)
    }

    /// LIST 明细列表（None = 当前目录）；目录项为小量元数据，内存聚合。
    pub async fn list(&mut self, path: Option<&str>) -> Result<Vec<Entry>, FtpError> {
        let body = self.read_data(list_line(path), DATA_OP_LIST).await?;
        Ok(parse_listing(&body))
    }

    /// NLST 名字列表（None = 当前目录）。
    pub async fn nlst(&mut self, path: Option<&str>) -> Result<Vec<String>, FtpError> {
        let body = self.read_data(nlst_line(path), DATA_OP_NLST).await?;
        Ok(body
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect())
    }

    /// RETR 下载：数据流分片直写 sink，返回实收字节数。
    pub async fn retr(
        &mut self,
        path: &str,
        sink: &mut (impl AsyncWrite + Unpin + Send),
    ) -> Result<u64, FtpError> {
        let mut data = self.open_data(format!("RETR {path}"), DATA_OP_GET).await?;
        let n = pump(&mut data, sink).await?;
        self.finish().await?;
        Ok(n)
    }

    /// STOR 上传（目标截断）。
    pub async fn stor(
        &mut self,
        path: &str,
        src: &mut (impl AsyncRead + Unpin + Send),
    ) -> Result<u64, FtpError> {
        self.put(path, src, false).await
    }

    /// APPE 上传（目标追加）。
    pub async fn appe(
        &mut self,
        path: &str,
        src: &mut (impl AsyncRead + Unpin + Send),
    ) -> Result<u64, FtpError> {
        self.put(path, src, true).await
    }

    pub async fn quit(mut self) -> Result<(), FtpError> {
        let r = self.cmd("QUIT").await?;
        self.require(&r, 221)
    }

    async fn put(
        &mut self,
        path: &str,
        src: &mut (impl AsyncRead + Unpin + Send),
        append: bool,
    ) -> Result<u64, FtpError> {
        let verb = if append { "APPE" } else { "STOR" };
        let mut data = self
            .open_data(format!("{verb} {path}"), DATA_OP_PUT)
            .await?;
        let n = tokio::io::copy(src, &mut data).await?;
        data.shutdown().await?;
        self.finish().await?;
        Ok(n)
    }

    /// 列表类传输：数据流聚合为文本（目录项元数据，量小）。
    async fn read_data(&mut self, line: String, op: u8) -> Result<String, FtpError> {
        let mut data = self.open_data(line, op).await?;
        let mut body = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = data.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&buf[..n]);
        }
        self.finish().await?;
        String::from_utf8(body).map_err(|e| FtpError::BadReply(format!("listing not utf-8: {e}")))
    }

    async fn open_data(&mut self, line: String, op: u8) -> Result<BoxedStream, FtpError> {
        let r = self.cmd(&line).await?;
        if r.code != 150 {
            return Err(FtpError::Rejected {
                code: r.code,
                text: r.text,
            });
        }
        let Some(token) = r.transfer_token() else {
            return Err(FtpError::BadReply(format!(
                "missing token in 150: {}",
                r.text
            )));
        };
        let mut data = self
            .node
            .new_stream(self.peer, proto(PROTO_DATA)?)
            .await
            .map_err(|e| FtpError::Assembly(e.to_string()))?;
        write_frame(&mut data, &data_header(op, &token)).await?;
        data.flush().await?;
        Ok(data)
    }

    async fn finish(&mut self) -> Result<(), FtpError> {
        let r = Reply::read(&mut self.ctrl).await?;
        if r.code == 226 {
            Ok(())
        } else {
            Err(FtpError::Rejected {
                code: r.code,
                text: r.text,
            })
        }
    }

    async fn cmd(&mut self, line: &str) -> Result<Reply, FtpError> {
        write_frame(&mut self.ctrl, line.as_bytes()).await?;
        Ok(Reply::read(&mut self.ctrl).await?)
    }

    fn require(&self, r: &Reply, code: u16) -> Result<(), FtpError> {
        if r.code == code {
            Ok(())
        } else {
            Err(FtpError::Rejected {
                code: r.code,
                text: r.text.clone(),
            })
        }
    }
}

fn list_line(path: Option<&str>) -> String {
    with_path("LIST", path)
}

fn nlst_line(path: Option<&str>) -> String {
    with_path("NLST", path)
}

fn with_path(verb: &str, path: Option<&str>) -> String {
    match path {
        Some(p) => format!("{verb} {p}"),
        None => verb.to_string(),
    }
}

/// 64KiB 分片泵：src 读到 EOF，逐片写 sink 并 flush。
async fn pump(
    src: &mut (impl AsyncRead + Unpin + Send),
    sink: &mut (impl AsyncWrite + Unpin + Send),
) -> io::Result<u64> {
    let mut buf = vec![0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = src.read(&mut buf).await?;
        if n == 0 {
            sink.flush().await?;
            return Ok(total);
        }
        sink.write_all(&buf[..n]).await?;
        total += n as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::super::wire::TOKEN_LEN;

    #[test]
    fn token_len_is_32() {
        assert_eq!(TOKEN_LEN, 32);
    }
}
