//! status 服务的线缆层：手写最小 HTTP/1.1 头解析、请求/请求体读取、响应写出
//! 与收尾排空。与业务路由（status.rs）分离；护栏常量随实现走，头多大都不信任。

use std::io;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// 请求头读取护栏。
const HEAD_CAP: usize = 8 * 1024;
/// 头/体读取超时。
const HEAD_TIMEOUT: Duration = Duration::from_secs(5);
/// 收尾排空上限：本地回环毫秒级，5s 是宽松护栏。
const DRAIN_TIMEOUT: Duration = Duration::from_secs(5);
/// /connect-share 请求体上限：链接为 KB 级，64 KiB 已宽松。
const BODY_CAP: usize = 64 * 1024;

/// 精确排空剩余请求体后再放连接：带未读数据关 socket 会触发 RST，可能竞掉
/// 已写出的响应（401 等未读 body 即回的路径，macOS 并行负载实证假红）。
/// 按 Content-Length 读满即走，不依赖对端关写，杜绝互等；读不全按异常放行并留痕。
pub(crate) async fn drain_exact(tcp: &mut TcpStream, remaining: usize) {
    if remaining == 0 {
        return;
    }
    let drain = async {
        let mut left = remaining;
        let mut buf = [0u8; 4096];
        while left > 0 {
            let n = tcp.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            left -= n.min(left);
        }
        Ok::<(), io::Error>(())
    };
    match tokio::time::timeout(DRAIN_TIMEOUT, drain).await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => tracing::debug!(error = %err, "status: drain read failed"),
        Err(_) => tracing::warn!(
            remaining,
            "status: drain timeout, dropping with unread input"
        ),
    }
}

/// 读请求（头至 "\r\n\r\n"）与同批到达的请求体前缀；护栏与超时，EOF/超限即坏请求。
pub(crate) async fn read_request(tcp: &mut TcpStream) -> io::Result<(String, Vec<u8>)> {
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
    let head_end = find_head_end(&buf)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unterminated head"))?;
    let head = String::from_utf8_lossy(&buf[..head_end + 4]).into_owned();
    Ok((head, buf[head_end + 4..].to_vec()))
}

/// 按 Content-Length 读全请求体（在 read_request 前缀上续读，带护栏）；
/// 缺长/超限/截断/超时即错，由路由方以 400 应答。
pub(crate) async fn read_body(
    tcp: &mut TcpStream,
    head: &str,
    prefix: Vec<u8>,
) -> io::Result<String> {
    let want = header_value(head, "content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing content-length"))?;
    if want > BODY_CAP {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "body too large"));
    }
    let mut buf = prefix;
    let read_rest = async {
        let mut chunk = [0u8; 1024];
        while buf.len() < want {
            let n = tcp.read(&mut chunk).await?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "truncated body",
                ));
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        Ok::<(), io::Error>(())
    };
    tokio::time::timeout(HEAD_TIMEOUT, read_rest)
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "body timeout"))??;
    Ok(String::from_utf8_lossy(&buf[..want]).into_owned())
}

fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

/// 取请求头值（大小写不敏感；take_while 空行止）。
pub(crate) fn header_value(head: &str, name: &str) -> Option<String> {
    head.split("\r\n")
        .skip(1)
        .take_while(|l| !l.is_empty())
        .find_map(|l| {
            let (n, v) = l.split_once(':')?;
            n.eq_ignore_ascii_case(name).then(|| v.trim().to_string())
        })
}

/// 解析请求行 + Authorization 头：只取方法、路径、Bearer 值，其余忽略。
pub(crate) fn parse_head(head: &str) -> (String, String, Option<String>) {
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

/// 写 JSON 响应并关写端（Connection: close；单请求连接语义）。
pub(crate) async fn reply(tcp: &mut TcpStream, status: u16, reason: &str, body: &str) {
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
