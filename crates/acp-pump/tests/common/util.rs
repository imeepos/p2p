//! status 契约测试的裸 HTTP 助手：不引依赖，按行拼头 + 读尽响应，
//! 返回完整响应文本（状态行/头/体一并交给测试断言）。

use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::STEP;

/// 裸 HTTP GET：返回完整响应文本。
pub async fn http_get(addr: SocketAddr, path: &str, token: Option<&str>) -> String {
    let head = match token {
        Some(t) => format!("GET {path} HTTP/1.1\r\nAuthorization: Bearer {t}\r\n\r\n"),
        None => format!("GET {path} HTTP/1.1\r\n\r\n"),
    };
    exchange(addr, &head, None).await
}

/// 裸 HTTP POST（JSON 体，带 Content-Length）：返回完整响应文本。
pub async fn http_post(addr: SocketAddr, path: &str, token: Option<&str>, body: &str) -> String {
    let auth = match token {
        Some(t) => format!("Authorization: Bearer {t}\r\n"),
        None => String::new(),
    };
    let head = format!(
        "POST {path} HTTP/1.1\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    exchange(addr, &head, Some(body)).await
}

async fn exchange(addr: SocketAddr, head: &str, body: Option<&str>) -> String {
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    s.write_all(head.as_bytes()).await.unwrap();
    if let Some(body) = body {
        s.write_all(body.as_bytes()).await.unwrap();
    }
    let mut buf = Vec::new();
    tokio::time::timeout(STEP, s.read_to_end(&mut buf))
        .await
        .unwrap()
        .unwrap();
    String::from_utf8(buf).unwrap()
}
