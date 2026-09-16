//! 数据通道执行面：按 DataKind 执行 GET/PUT/APPE/LIST 传输。
//!
//! PUT 走 HiddenStores（ProFTPD 同名机制）：先写同目录隐藏临时文件，成功后
//! 原子 rename 到目标，任何失败自动清临时文件——目标路径永不出现半截文件；
//! APPE 直写目标（追加语义=断点续传友好，vsftpd 先例），保留 partial 行为。

use std::io;

use p2p_mux::BoxedStream;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::server::FtpServer;
use crate::transfer::DataKind;
use crate::vfs::format_listing;

pub(crate) async fn run_data_transfer(
    server: &FtpServer,
    kind: DataKind,
    vpath: &str,
    stream: &mut BoxedStream,
) -> io::Result<u64> {
    match kind {
        DataKind::Get => run_get(server, vpath, stream).await,
        DataKind::Put => run_put(server, vpath, stream).await,
        DataKind::Append => run_append(server, vpath, stream).await,
        DataKind::List | DataKind::Nlst => run_list(server, kind, vpath, stream).await,
    }
}

async fn run_get(server: &FtpServer, vpath: &str, stream: &mut BoxedStream) -> io::Result<u64> {
    let mut reader = server.fs().reader(vpath).await?;
    let n = tokio::io::copy(&mut reader, stream).await?;
    stream.flush().await?;
    stream.shutdown().await?;
    Ok(n)
}

/// STOR 走 HiddenStores（ProFTPD 同名机制）：先写同目录隐藏临时文件，成功后
/// 原子 rename 到目标，任何失败自动清临时文件——目标路径永不出现半截文件。
async fn run_put(server: &FtpServer, vpath: &str, stream: &mut BoxedStream) -> io::Result<u64> {
    let partial = partial_path(vpath).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "stor target has no file name")
    })?;
    let mut writer = server.fs().writer(&partial, false).await?;
    let staged = copy_limited(stream, &mut writer, server.cfg().max_upload_bytes).await;
    if let Err(e) = staged {
        cleanup_partial(server, &partial, &e).await;
        return Err(e);
    }
    let n = staged.unwrap_or(0);
    if let Err(e) = writer.flush().await {
        cleanup_partial(server, &partial, &e).await;
        return Err(e);
    }
    if let Err(e) = server.fs().rename(&partial, vpath).await {
        cleanup_partial(server, &partial, &e).await;
        return Err(e);
    }
    Ok(n)
}

/// APPE 直写目标（追加语义=断点续传友好，vsftpd 先例），保留 partial 行为。
async fn run_append(server: &FtpServer, vpath: &str, stream: &mut BoxedStream) -> io::Result<u64> {
    let mut writer = server.fs().writer(vpath, true).await?;
    let n = copy_limited(stream, &mut writer, server.cfg().max_upload_bytes).await?;
    writer.flush().await?;
    Ok(n)
}

/// 失败清场：临时文件必删；删除再失败只留告警（磁盘残留可观测）。
async fn cleanup_partial(server: &FtpServer, partial: &str, cause: &io::Error) {
    if let Err(cleanup) = server.fs().remove_file(partial).await {
        tracing::warn!(partial = %partial, cause = %cause, error = %cleanup, "ftp staged partial cleanup failed");
    }
}

async fn run_list(
    server: &FtpServer,
    kind: DataKind,
    vpath: &str,
    stream: &mut BoxedStream,
) -> io::Result<u64> {
    let entries = server.fs().list(vpath).await?;
    let body = match kind {
        DataKind::Nlst => entries
            .into_iter()
            .map(|e| format!("{}\n", e.name))
            .collect::<String>(),
        _ => format_listing(&entries),
    };
    stream.write_all(body.as_bytes()).await?;
    stream.flush().await?;
    stream.shutdown().await?;
    Ok(body.len() as u64)
}

async fn copy_limited(
    src: &mut BoxedStream,
    dst: &mut (impl AsyncWrite + Unpin + Send),
    max: u64,
) -> io::Result<u64> {
    let mut buf = vec![0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = src.read(&mut buf).await?;
        if n == 0 {
            return Ok(total);
        }
        total += n as u64;
        if total > max {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                format!("upload exceeds configured limit of {max} bytes"),
            ));
        }
        dst.write_all(&buf[..n]).await?;
    }
}

/// 同目录隐藏临时路径：`/docs/big.bin` → `/docs/.big.bin.p2p-ftp-partial`。
/// 同目录保证 rename 原子（同一文件系统）；无名路径（根目录本身）不可 STOR。
fn partial_path(vpath: &str) -> Option<String> {
    let (dir, name) = vpath.rsplit_once('/')?;
    if name.is_empty() {
        return None;
    }
    let dir = if dir.is_empty() { "" } else { dir };
    Some(format!("{dir}/.{name}{PARTIAL_SUFFIX}"))
}

/// 隐藏临时文件后缀（同目录内避开常规列表干扰）。
pub(crate) const PARTIAL_SUFFIX: &str = ".p2p-ftp-partial";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_path_same_dir_hidden_name() {
        assert_eq!(
            partial_path("/docs/big.bin"),
            Some(format!("/docs/.big.bin{PARTIAL_SUFFIX}"))
        );
        assert_eq!(
            partial_path("/a.bin"),
            Some(format!("/.a.bin{PARTIAL_SUFFIX}"))
        );
    }

    #[test]
    fn partial_path_rejects_nameless_targets() {
        assert_eq!(partial_path("/"), None);
        assert_eq!(partial_path("/docs/"), None);
    }
}
