//! 进程内 mock 上游 HTTP 服务（B1/B5）：回环监听，记录 path/鉴权头/请求体，
//! 按剧本逐请求回 SSE 字节；真实 HttpUpstream/ClaudeUpstream 指向本进程，不出网。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// 一次上游请求的观测记录（泄露面断言的数据源）。
#[derive(Debug, Clone)]
pub struct RecordedRequest {
    #[allow(dead_code)] // 观测留档：泄露断言只读鉴权头与 body
    pub path: String,
    pub authorization: Option<String>,
    pub x_api_key: Option<String>,
    pub body: String,
}

/// mock 上游：SSE 剧本逐请求弹出，弹尽回 500（缺省拒绝不悬挂）。
pub struct MockHttpUpstream {
    base: String,
    requests: Mutex<Vec<RecordedRequest>>,
    script: Mutex<VecDeque<Vec<u8>>>,
}

impl MockHttpUpstream {
    pub async fn start(script: Vec<Vec<u8>>) -> Arc<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let base = format!("http://{}", listener.local_addr().expect("local addr"));
        let this = Arc::new(Self {
            base,
            requests: Mutex::new(Vec::new()),
            script: Mutex::new(script.into_iter().collect()),
        });
        let bg = this.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((sock, _)) => bg.serve_conn(sock).await,
                    Err(e) => {
                        eprintln!("mock upstream accept 失败，监听终止: {e}");
                        break;
                    }
                }
            }
        });
        this
    }

    pub fn base(&self) -> String {
        self.base.clone()
    }

    pub fn recorded(&self) -> Vec<RecordedRequest> {
        self.requests.lock().expect("requests lock").clone()
    }

    pub fn request_count(&self) -> usize {
        self.requests.lock().expect("requests lock").len()
    }

    async fn serve_conn(&self, mut sock: TcpStream) {
        let mut buf: Vec<u8> = Vec::with_capacity(4096);
        let Some(head_end) = read_head(&mut sock, &mut buf).await else {
            return;
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
        let want = content_length(&head);
        while buf.len() < head_end + 4 + want {
            let mut chunk = [0u8; 4096];
            match sock.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
        }
        let body_start = (head_end + 4).min(buf.len());
        let body_end = (body_start + want).min(buf.len());
        let request = RecordedRequest {
            path: request_path(&head),
            authorization: header_value(&head, "authorization"),
            x_api_key: header_value(&head, "x-api-key"),
            body: String::from_utf8_lossy(&buf[body_start..body_end]).into_owned(),
        };
        self.requests.lock().expect("requests lock").push(request);
        let reply = match self.script.lock().expect("script lock").pop_front() {
            Some(sse) => http_ok(&sse),
            None => http_status(500, b"mock script exhausted"),
        };
        let _ = sock.write_all(&reply).await;
        let _ = sock.shutdown().await;
    }
}

async fn read_head(sock: &mut TcpStream, buf: &mut Vec<u8>) -> Option<usize> {
    let mut chunk = [0u8; 4096];
    loop {
        if let Some(pos) = find_head_end(buf) {
            return Some(pos);
        }
        match sock.read(chunk.as_mut()).await {
            Ok(0) | Err(_) => return None,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
}

/// 头部终止 = 空行分隔（\r\n\r\n），返回其起始偏移。
fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn content_length(head: &str) -> usize {
    head.lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0)
}

fn request_path(head: &str) -> String {
    head.lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/")
        .to_owned()
}

fn header_value(head: &str, name: &str) -> Option<String> {
    head.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_owned())
    })
}

fn http_ok(sse: &[u8]) -> Vec<u8> {
    http_status(200, sse)
}

fn http_status(code: u16, body: &[u8]) -> Vec<u8> {
    let mut reply = format!(
        "HTTP/1.1 {code} MOCK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    reply.extend_from_slice(body);
    reply
}
