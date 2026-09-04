//! 本地 status 端点（需求 C 的查询面，GUI 波依赖；README 为契约权威）：
//! GET /status 返回连接状态机快照，GET /discovery 返回发现候选清单；
//! Bearer token 鉴权，绑 127.0.0.1。手写最小 HTTP/1.1 头解析，不引服务端框架。

use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::discovery::DiscoveryHub;
use crate::state::StatusHub;

/// 请求头读取护栏：头多大都不信任。
const HEAD_CAP: usize = 8 * 1024;
const HEAD_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct StatusDeps {
    pub hub: Arc<StatusHub>,
    pub discovery: Arc<DiscoveryHub>,
}

pub struct StatusServer {
    pub addr: SocketAddr,
}

impl StatusServer {
    pub async fn start(port: u16, token: String, deps: StatusDeps) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
        let addr = listener.local_addr()?;
        tokio::spawn(accept_loop(listener, token, deps));
        Ok(Self { addr })
    }
}

async fn accept_loop(listener: TcpListener, token: String, deps: StatusDeps) {
    loop {
        match listener.accept().await {
            Ok((tcp, peer_addr)) => {
                if !peer_addr.ip().is_loopback() {
                    tracing::warn!(%peer_addr, "status client from non-loopback rejected");
                    continue;
                }
                tokio::spawn(serve_conn(tcp, token.clone(), deps.clone()));
            }
            Err(err) => {
                tracing::warn!(error = %err, "status accept failed");
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn serve_conn(mut tcp: TcpStream, token: String, deps: StatusDeps) {
    let head = match read_head(&mut tcp).await {
        Ok(head) => head,
        Err(err) => {
            tracing::warn!(error = %err, "status: bad request head");
            return;
        }
    };
    let (method, path, bearer) = parse_head(&head);
    if bearer.as_deref() != Some(token.as_str()) {
        tracing::warn!(%path, "status: unauthorized request");
        reply(
            &mut tcp,
            401,
            "Unauthorized",
            "{\"error\":\"unauthorized\"}",
        )
        .await;
        return;
    }
    match (method.as_str(), path.as_str()) {
        ("GET", "/status") => {
            let body = serde_json::to_string(&deps.hub.snapshot())
                .unwrap_or_else(|_| "{\"error\":\"serialize\"}".into());
            reply(&mut tcp, 200, "OK", &body).await;
        }
        ("GET", "/discovery") => {
            let snapshot = deps.discovery.snapshot();
            let body = serde_json::to_string(&Value::Object(
                [(
                    "peers".to_string(),
                    serde_json::to_value(snapshot).unwrap_or(Value::Null),
                )]
                .into_iter()
                .collect(),
            ))
            .unwrap_or_else(|_| "{\"error\":\"serialize\"}".into());
            reply(&mut tcp, 200, "OK", &body).await;
        }
        _ => reply(&mut tcp, 404, "Not Found", "{\"error\":\"not-found\"}").await,
    }
}

/// 读请求头（至 "\r\n\r\n"），带护栏与超时；EOF/超限即坏请求。
async fn read_head(tcp: &mut TcpStream) -> io::Result<String> {
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
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "status head timeout"))??;
    if find_head_end(&buf).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unterminated head",
        ));
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

/// 解析请求行 + Authorization 头：只取方法、路径、Bearer 值，其余忽略。
fn parse_head(head: &str) -> (String, String, Option<String>) {
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let bearer = lines.take_while(|l| !l.is_empty()).find_map(|l| {
        let (name, value) = l.split_once(':')?;
        name.eq_ignore_ascii_case("authorization")
            .then(|| value.trim().strip_prefix("Bearer ").map(str::to_string))
            .flatten()
    });
    (method, path, bearer)
}

async fn reply(tcp: &mut TcpStream, status: u16, reason: &str, body: &str) {
    let resp = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    if let Err(err) = tcp.write_all(resp.as_bytes()).await {
        tracing::warn!(error = %err, "status: reply write failed");
        return;
    }
    if let Err(err) = tcp.shutdown().await {
        tracing::debug!(error = %err, "status: shutdown failed");
    }
}
