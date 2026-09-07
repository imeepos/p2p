//! owner 本地管理 HTTP 管道（设计 §5）：只绑 127.0.0.1，Bearer token 鉴权，
//! 手写最小 HTTP/1.1（沿 acp-console status 端点先例，不引服务端框架）。
//! 端点语义在 api.rs；token 原文永不进日志：本模块只比较不打印。

use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::{LinkContext, ShareService};

/// 请求头/请求体护栏：多大都不信任。
const HEAD_CAP: usize = 8 * 1024;
const BODY_CAP: usize = 64 * 1024;
const HEAD_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct AdminDeps {
    pub service: Arc<ShareService>,
    pub link: LinkContext,
}

/// Bearer 凭据：随机生成，落 <data-dir>/acp-admin-token（0600）。
pub struct AdminToken {
    pub value: String,
    pub file: PathBuf,
}

impl AdminToken {
    pub fn issue(path: PathBuf) -> io::Result<Self> {
        let value = super::generate_token();
        acp_common::write_private_file(&path, value.as_bytes())?;
        Ok(Self { value, file: path })
    }
}

pub struct AdminServer {
    pub addr: SocketAddr,
}

impl AdminServer {
    pub async fn start(port: u16, token: String, deps: AdminDeps) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
        let addr = listener.local_addr()?;
        tokio::spawn(accept_loop(listener, token, deps));
        Ok(Self { addr })
    }
}

/// 启动 stdout JSON 行（设计 §5 冻结形态），owner 本机可观测。
pub fn ready_line(port: u16, token_file: &std::path::Path) -> String {
    format!(
        "{{\"kind\":\"ready\",\"admin\":{{\"port\":{port},\"token_file\":\"{}\"}}}}",
        token_file.display()
    )
}

async fn accept_loop(listener: TcpListener, token: String, deps: AdminDeps) {
    loop {
        match listener.accept().await {
            Ok((tcp, peer_addr)) => {
                if !peer_addr.ip().is_loopback() {
                    tracing::warn!(%peer_addr, "admin client from non-loopback rejected");
                    continue;
                }
                tokio::spawn(serve_conn(tcp, token.clone(), deps.clone()));
            }
            Err(err) => {
                tracing::warn!(error = %err, "admin accept failed");
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn serve_conn(mut tcp: TcpStream, token: String, deps: AdminDeps) {
    let buf = match read_head(&mut tcp).await {
        Ok(buf) => buf,
        Err(err) => {
            tracing::warn!(error = %err, "admin: bad request head");
            return;
        }
    };
    let Some(head_end) = find_head_end(&buf) else {
        tracing::warn!("admin: head end missing after read");
        return;
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    // head 之后的同批字节是 body 前缀，read_head 不得丢弃。
    let mut pending: Vec<u8> = buf[head_end + 4..].to_vec();
    let (method, target, bearer, content_length) = parse_head(&head);
    if bearer.as_deref() != Some(token.as_str()) {
        tracing::warn!(%target, "admin: unauthorized request");
        reply(
            &mut tcp,
            401,
            "Unauthorized",
            "{\"error\":\"unauthorized\"}",
        )
        .await;
        return;
    }
    let body = match read_body(&mut tcp, content_length, &mut pending).await {
        Ok(body) => body,
        Err(err) => {
            tracing::warn!(error = %err, "admin: bad request body");
            reply(&mut tcp, 400, "Bad Request", "{\"error\":\"invalid-body\"}").await;
            return;
        }
    };
    super::api::route(&mut tcp, &deps, &method, &target, &body).await;
}

/// 读请求头（至 "\r\n\r\n"），带护栏与超时；EOF/超限即坏请求。
/// 返回整段缓冲（含 body 前缀），由调用方切分，避免吞掉同批到达的 body。
async fn read_head(tcp: &mut TcpStream) -> io::Result<Vec<u8>> {
    let inner = async {
        let mut buf: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 512];
        loop {
            if find_head_end(&buf).is_some() || buf.len() > HEAD_CAP {
                break;
            }
            let n = tcp.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        Ok::<Vec<u8>, io::Error>(buf)
    };
    let buf = tokio::time::timeout(HEAD_TIMEOUT, inner)
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "admin head timeout"))??;
    if find_head_end(&buf).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unterminated head",
        ));
    }
    Ok(buf)
}

fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

/// 按 Content-Length 精确读体；先消费 head 之后已到达的前缀，缺额再补读。
async fn read_body(
    tcp: &mut TcpStream,
    content_length: Option<usize>,
    pending: &mut Vec<u8>,
) -> io::Result<Vec<u8>> {
    let Some(len) = content_length else {
        return Ok(Vec::new());
    };
    if len > BODY_CAP {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "body too large"));
    }
    if pending.len() < len {
        let missing = len - pending.len();
        pending.resize(len, 0);
        tcp.read_exact(&mut pending[len - missing..]).await?;
    }
    pending.truncate(len);
    Ok(std::mem::take(pending))
}

/// 解析请求行 + Authorization/Content-Length 头：只取方法、路径、Bearer 与体长。
fn parse_head(head: &str) -> (String, String, Option<String>, Option<usize>) {
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let mut bearer = None;
    let mut content_length = None;
    for line in lines.take_while(|l| !l.is_empty()) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("authorization") {
            bearer = value.strip_prefix("Bearer ").map(str::to_string);
        }
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.parse().ok();
        }
    }
    (method, path, bearer, content_length)
}

pub(super) async fn reply_json(
    tcp: &mut TcpStream,
    status: u16,
    reason: &str,
    body: &serde_json::Value,
) {
    reply(tcp, status, reason, &body.to_string()).await;
}

pub(super) async fn reply(tcp: &mut TcpStream, status: u16, reason: &str, body: &str) {
    let resp = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    if let Err(err) = tcp.write_all(resp.as_bytes()).await {
        tracing::warn!(error = %err, "admin: reply write failed");
        return;
    }
    if let Err(err) = tcp.shutdown().await {
        tracing::debug!(error = %err, "admin: shutdown failed");
    }
}
