//! 最小 HTTP/1.1 服务器：仅服务本机挂载桥（默认绑 127.0.0.1）。
//!
//! 单请求单连接：应答恒 `Connection: close`（规避 keep-alive 状态机缺陷面）；
//! 请求头解析限时防慢速挂连；支持 Expect: 100-continue、Content-Length 与
//! chunked 请求体。只覆盖 WebDAV 桥需要的语义，不是通用 web 框架。

pub mod body;
pub mod response;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::net::{TcpListener, TcpStream};

use body::BodyReader;
use response::write_response;
pub use response::{HttpResponse, ResponseBody};

/// 请求头解析与首读限时：慢速客户端不占桥资源。
const HEAD_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_HEAD_BYTES: usize = 32 * 1024;

#[derive(Debug)]
pub struct HttpRequestHead {
    pub method: String,
    /// 原始请求目标（含 query，未解码）。
    pub target: String,
    pub headers: Vec<(String, String)>,
}

impl HttpRequestHead {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

pub struct HttpRequest {
    head: HttpRequestHead,
    pub body: BodyReader,
}

impl HttpRequest {
    pub fn new(head: HttpRequestHead, body: BodyReader) -> Self {
        Self { head, body }
    }

    pub fn method(&self) -> &str {
        &self.head.method
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.head.header(name)
    }

    /// percent 解码后的路径（去 query）。
    pub fn path(&self) -> String {
        let raw = self.head.target.split('?').next().unwrap_or("");
        percent_decode(raw)
    }
}

#[async_trait]
pub trait HttpHandler: Send + Sync {
    async fn respond(&self, req: HttpRequest) -> HttpResponse;
}

/// accept 循环：每连接一个任务，handler panic/错误仅计日志不断服务；
/// shutdown 翻 true（或发送端 drop）即退出。
pub async fn serve(
    listener: TcpListener,
    handler: Arc<dyn HttpHandler>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            _ = wait_shutdown(&mut shutdown) => {
                tracing::info!("http bridge shutting down");
                return;
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, addr)) => {
                        let handler = Arc::clone(&handler);
                        tokio::spawn(async move {
                            if let Err(e) = handle_conn(stream, handler).await {
                                tracing::debug!(%addr, "vdrive bridge conn ended: {e}");
                            }
                        });
                    }
                    Err(e) => {
                        tracing::warn!("vdrive bridge accept failed: {e}");
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        }
    }
}

async fn wait_shutdown(rx: &mut tokio::sync::watch::Receiver<bool>) {
    if *rx.borrow_and_update() {
        return;
    }
    // 发送端消失 = 宿主已收口，同步退出。
    let _ = rx.changed().await;
}

async fn handle_conn(stream: TcpStream, handler: Arc<dyn HttpHandler>) -> std::io::Result<()> {
    stream.set_nodelay(true).ok();
    let (rd, mut wr) = stream.into_split();
    let parsed = tokio::time::timeout(HEAD_TIMEOUT, read_request(rd)).await;
    let req = match parsed {
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "request head timeout",
            ));
        }
        Ok(r) => r?,
    };
    let Some(req) = req else {
        return Ok(());
    };
    if expects_continue(&req) {
        use tokio::io::AsyncWriteExt;
        wr.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").await?;
        wr.flush().await?;
    }
    let resp = handler.respond(req).await;
    write_response(&mut wr, resp).await
}

fn expects_continue(req: &HttpRequest) -> bool {
    req.header("expect")
        .map(|v| v.eq_ignore_ascii_case("100-continue"))
        .unwrap_or(false)
}

/// 读请求头并装配带流式请求体的请求；rd 按值接管（body 归请求所有）。
async fn read_request<R: AsyncRead + Unpin + Send + 'static>(
    mut rd: R,
) -> std::io::Result<Option<HttpRequest>> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = rd.read(&mut byte).await?;
        if n == 0 {
            return if head.is_empty() {
                Ok(None)
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "request head truncated",
                ))
            };
        }
        head.push(byte[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
        if head.len() > MAX_HEAD_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request head too large",
            ));
        }
    }
    parse_request(head, rd).map(Some)
}

fn parse_request<R: AsyncRead + Unpin + Send + 'static>(
    head: Vec<u8>,
    rd: R,
) -> std::io::Result<HttpRequest> {
    let text = String::from_utf8(head)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "head not utf-8"))?;
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "missing method"))?;
    let target = parts
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "missing target"))?;
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    let lookup = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    };
    let chunked = lookup("transfer-encoding")
        .map(|v| v.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false);
    let body = if chunked {
        BodyReader::from_stream(Box::new(rd), None)
    } else {
        let len = lookup("content-length")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        BodyReader::from_stream(Box::new(rd), Some(len))
    };
    Ok(HttpRequest::new(
        HttpRequestHead {
            method: method.to_string(),
            target: target.to_string(),
            headers,
        },
        body,
    ))
}

/// percent 解码（仅 %XX；路径场景 `+` 不还原为空格）。
pub fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(hex) = bytes.get(i + 1..i + 3) {
                if let Ok(v) = u8::from_str_radix(&String::from_utf8_lossy(hex), 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_plain_and_percent() {
        assert_eq!(percent_decode("/a/b.txt"), "/a/b.txt");
        assert_eq!(percent_decode("/a%20b/c"), "/a b/c");
        assert_eq!(percent_decode("/%E4%B8%AD.txt"), "/中.txt");
        assert_eq!(percent_decode("/bad%zz"), "/bad%zz");
    }
}
