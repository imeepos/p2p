//! WebSocket 升级路径反代单测（迁移自 proxy_tests.rs，逐字）。

use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::*;

/// 目标服务（ws 用例）：断言升级头，回 101（带首包 srv），随后帧面回显。
async fn target_ws_echo(target: TcpListener) {
    let (mut conn, _) = target.accept().await.expect("accept");
    let mut chunk = [0u8; 4096];
    tunnel_skin(&mut conn).await;
    let req = read_bytes_until(&mut conn, Vec::new(), is_complete_request).await;
    let head = String::from_utf8_lossy(&req).into_owned();
    assert!(head.contains("Upgrade: websocket"), "升级头丢失: {head}");
    assert!(
        head.contains("Connection: Upgrade"),
        "Connection 被改写: {head}"
    );
    assert!(
        head.contains("Origin: http://127.0.0.1:3080"),
        "升级请求 Origin 未重写: {head}"
    );
    conn.write_all(
        b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\nsrv",
    )
    .await
    .expect("101");
    loop {
        let read = conn.read(&mut chunk).await.expect("echo read");
        if read == 0 {
            return;
        }
        conn.write_all(&chunk[..read]).await.expect("echo write");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn websocket_upgrade_pumps_raw_bytes_both_ways() {
    let target = TcpListener::bind(("127.0.0.1", 0)).await.expect("target");
    let proxy_addr = spawn_proxy(
        Arc::new(ManualOpener {
            addr: target.local_addr().unwrap(),
            target: "127.0.0.1:3080".into(),
        }),
        3080,
    )
    .await;
    tokio::spawn(target_ws_echo(target));

    let mut conn = TcpStream::connect(proxy_addr).await.expect("connect");
    conn.write_all(b"GET /api/remote.mux HTTP/1.1\r\nHost: 127.0.0.1:3080\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nOrigin: http://127.0.0.1:52172\r\nSec-WebSocket-Key: aGVsbG8=\r\nSec-WebSocket-Version: 13\r\n\r\n")
        .await
        .expect("req");
    let mut head = Vec::new();
    let mut one = [0u8; 1];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        let read = conn.read(&mut one).await.expect("101 read");
        assert!(read > 0, "EOF before 101");
        head.push(one[0]);
    }
    assert!(head.starts_with(b"HTTP/1.1 101"), "非 101: {head:?}");
    let mut buf = [0u8; 16];
    conn.read_exact(&mut buf[..3]).await.expect("srv push");
    assert_eq!(&buf[..3], b"srv", "101 后首包字节丢失");
    conn.write_all(b"abc").await.expect("c2s");
    conn.read_exact(&mut buf[..3]).await.expect("echo");
    assert_eq!(&buf[..3], b"abc");
}
