//! HTTP（非升级）路径反代单测（迁移自 proxy_tests.rs，逐字）。

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::*;

/// 目标服务（host 用例）：收完整请求 → 断言 Host/Connection → 分两拍响应。
async fn target_host_and_stream(target: TcpListener, expect_port: u16) {
    let (mut conn, _) = target.accept().await.expect("accept");
    tunnel_skin(&mut conn).await;
    let buf = read_bytes_until(&mut conn, Vec::new(), |b| {
        is_complete_request(b) && b.ends_with(b"hello")
    })
    .await;
    let head = String::from_utf8_lossy(&buf).into_owned();
    assert!(
        head.contains(&format!("Host: 127.0.0.1:{expect_port}")),
        "Host 未重写: {head}"
    );
    assert!(
        head.contains("Connection: close"),
        "hop-by-hop 未处理: {head}"
    );
    conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\n01234")
        .await
        .expect("write1");
    tokio::time::sleep(Duration::from_millis(200)).await;
    conn.write_all(b"56789").await.expect("write2");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn host_rewritten_and_response_streams_incrementally() {
    let target = TcpListener::bind(("127.0.0.1", 0)).await.expect("target");
    let target_port = target.local_addr().unwrap().port();
    let proxy_addr = spawn_proxy(
        Arc::new(ManualOpener {
            addr: target.local_addr().unwrap(),
            target: format!("127.0.0.1:{target_port}"),
        }),
        target_port,
    )
    .await;
    let t = tokio::spawn(target_host_and_stream(target, target_port));

    let mut conn = TcpStream::connect(proxy_addr).await.expect("connect");
    conn.write_all(b"POST /x HTTP/1.1\r\nHost: 127.0.0.1:1\r\nContent-Length: 5\r\n\r\nhello")
        .await
        .expect("req");
    let mut first_at = None;
    let mut body = Vec::new();
    let mut chunk = [0u8; 64];
    loop {
        let read = conn.read(&mut chunk).await.expect("read");
        if read == 0 {
            break;
        }
        if first_at.is_none() {
            first_at = Some(Instant::now());
        }
        body.extend_from_slice(&chunk[..read]);
    }
    let full_at = Instant::now();
    let text = String::from_utf8_lossy(&body).into_owned();
    assert!(text.starts_with("HTTP/1.1 200 OK"), "响应头异常: {text}");
    assert!(text.ends_with("0123456789"), "body 不完整: {text}");
    let gap = full_at.duration_since(first_at.expect("first byte"));
    assert!(
        gap >= Duration::from_millis(150),
        "响应疑似整包缓冲（首尾间隔 {gap:?}）"
    );
    t.await.expect("target task");
}

/// 目标服务（POST 用例）：断言 Host/Origin/Referer 全部重写为目标 authority。
async fn target_origin_assert(target: TcpListener, expect_port: u16) {
    let (mut conn, _) = target.accept().await.expect("accept");
    tunnel_skin(&mut conn).await;
    let buf = read_bytes_until(&mut conn, Vec::new(), |b| {
        is_complete_request(b) && b.ends_with(b"{}")
    })
    .await;
    let head = String::from_utf8_lossy(&buf).into_owned();
    let authority = format!("http://127.0.0.1:{expect_port}");
    assert!(
        head.contains(&format!("Host: 127.0.0.1:{expect_port}")),
        "Host 未重写: {head}"
    );
    assert!(
        head.contains(&format!("Origin: {authority}")),
        "Origin 未重写: {head}"
    );
    assert!(
        head.contains(&format!("Referer: {authority}/?token=t")),
        "Referer 未重写: {head}"
    );
    conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
        .await
        .expect("write");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn post_origin_referer_rewritten_end_to_end() {
    let target = TcpListener::bind(("127.0.0.1", 0)).await.expect("target");
    let target_port = target.local_addr().unwrap().port();
    let proxy_addr = spawn_proxy(
        Arc::new(ManualOpener {
            addr: target.local_addr().unwrap(),
            target: format!("127.0.0.1:{target_port}"),
        }),
        target_port,
    )
    .await;
    let t = tokio::spawn(target_origin_assert(target, target_port));
    let mut conn = TcpStream::connect(proxy_addr).await.expect("connect");
    let req = format!(
        "POST /api/session HTTP/1.1\r\nHost: 127.0.0.1:1\r\n\
         Origin: http://127.0.0.1:{proxy_port}\r\n\
         Referer: http://127.0.0.1:{proxy_port}/?token=t\r\n\
         Content-Type: application/json\r\nContent-Length: 2\r\n\r\n{{}}",
        proxy_port = proxy_addr.port()
    );
    conn.write_all(req.as_bytes()).await.expect("req");
    let mut body = Vec::new();
    conn.read_to_end(&mut body).await.expect("read");
    assert!(
        body.starts_with(b"HTTP/1.1 200"),
        "非 200: {}",
        String::from_utf8_lossy(&body)
    );
    t.await.expect("target task");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unreachable_target_yields_502_with_reason() {
    let proxy_addr = spawn_proxy(Arc::new(DeadOpener), 3080).await;
    let mut conn = TcpStream::connect(proxy_addr).await.expect("connect");
    conn.write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1:3080\r\n\r\n")
        .await
        .expect("req");
    let mut body = Vec::new();
    conn.read_to_end(&mut body).await.expect("read");
    let text = String::from_utf8_lossy(&body).into_owned();
    assert!(text.starts_with("HTTP/1.1 502"), "非 502: {text}");
    assert!(text.contains("target down"), "失败原因未透出: {text}");
}
