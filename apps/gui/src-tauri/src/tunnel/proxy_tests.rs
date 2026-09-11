//! 反代行为单测：真实回环 TCP 对端（禁 mock 整包缓冲的面）验证 Host 重写、
//! 流式转发、升级裸泵与拒绝路径。隧道字节面由 client.rs 单测覆盖。

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::LocalProxy;
use crate::tunnel::client::TunnelDialer;
use p2p::BoxedStream;

/// 直连被测目标服务的拨流器（替身=真实 TCP **裸流**；协议 ID/票据握手由
/// open_tunnel 唯一写入，对端隧道皮见 tunnel_skin——与生产 NodeDialer 同构）。
struct TargetDialer {
    addr: std::net::SocketAddr,
}

#[async_trait::async_trait]
impl TunnelDialer for TargetDialer {
    async fn dial(&self) -> Result<BoxedStream, String> {
        Ok(Box::new(
            TcpStream::connect(self.addr).await.map_err(|e| e.to_string())?,
        ) as BoxedStream)
    }
}

/// 拨流器恒失败（隧道不可达）。
struct DeadDialer;

#[async_trait::async_trait]
impl TunnelDialer for DeadDialer {
    async fn dial(&self) -> Result<BoxedStream, String> {
        Err("target down".into())
    }
}

async fn spawn_proxy(dialer: Arc<dyn TunnelDialer>, target_port: u16) -> std::net::SocketAddr {
    let proxy = LocalProxy::bind(target_port, "test-peer".into(), dialer)
        .await
        .expect("bind");
    let addr = proxy.local_addr();
    tokio::spawn(proxy.serve());
    addr
}

/// 目标服务：收满请求后断言 Host/Connection 头，再分两拍发响应体。
/// 隧道皮：accept 后读协议 ID 帧 + 票据帧（严格序），回 ack，再交 HTTP 体。
async fn tunnel_skin(
    conn: &mut TcpStream,
) {
    let proto = p2p_protocol::read_frame(conn).await.expect("proto frame");
    assert_eq!(
        std::str::from_utf8(&proto).expect("utf8"),
        crate::tunnel::ticket::PROTOCOL_ID,
        "流上首帧必须是协议 ID（双写装配检测）"
    );
    let ticket = p2p_protocol::read_frame(conn).await.expect("ticket frame");
    let ticket: serde_json::Value =
        serde_json::from_slice(&ticket).expect("ticket json");
    // 真实 responder 语义：ack 回显同 uid（§3）。
    let ack = serde_json::json!({"k":"ack","uid": ticket["uid"]});
    p2p_protocol::write_frame(conn, &serde_json::to_vec(&ack).expect("ack"))
        .await
        .expect("ack write");
}

async fn target_host_and_stream(target: TcpListener, expect_port: u16) {
    let (mut conn, _) = target.accept().await.expect("accept");
    tunnel_skin(&mut conn).await;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let read = conn.read(&mut chunk).await.expect("read");
        buf.extend_from_slice(&chunk[..read]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.ends_with(b"hello") {
            break;
        }
    }
    let head = String::from_utf8_lossy(&buf).into_owned();
    assert!(
        head.contains(&format!("Host: 127.0.0.1:{expect_port}")),
        "Host 未重写: {head}"
    );
    assert!(head.contains("Connection: close"), "hop-by-hop 未处理: {head}");
    conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\n01234")
        .await
        .expect("write1");
    conn.flush().await.expect("flush");
    tokio::time::sleep(Duration::from_millis(200)).await;
    conn.write_all(b"56789").await.expect("write2");
    conn.flush().await.expect("flush");
}

#[tokio::test]
async fn host_rewritten_and_response_streams_incrementally() {
    let target = TcpListener::bind(("127.0.0.1", 0)).await.expect("target");
    let target_port = target.local_addr().unwrap().port();
    let proxy_addr = spawn_proxy(Arc::new(TargetDialer { addr: target.local_addr().unwrap() }), target_port).await;
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

/// 目标服务：断言升级头，回 101，然后裸字节回显。
async fn target_ws_echo(target: TcpListener) {
    let (mut conn, _) = target.accept().await.expect("accept");
    tunnel_skin(&mut conn).await;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let read = conn.read(&mut chunk).await.expect("read");
        buf.extend_from_slice(&chunk[..read]);
    }
    let head = String::from_utf8_lossy(&buf).into_owned();
    assert!(head.contains("Upgrade: websocket"), "升级头丢失: {head}");
    assert!(head.contains("Connection: Upgrade"), "Connection 被改写: {head}");
    conn.write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\nsrv")
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

#[tokio::test]
async fn websocket_upgrade_pumps_raw_bytes_both_ways() {
    let target = TcpListener::bind(("127.0.0.1", 0)).await.expect("target");
    let proxy_addr = spawn_proxy(Arc::new(TargetDialer { addr: target.local_addr().unwrap() }), 3080).await;
    tokio::spawn(target_ws_echo(target));

    let mut conn = TcpStream::connect(proxy_addr).await.expect("connect");
    conn.write_all(b"GET /api/remote.mux HTTP/1.1\r\nHost: 127.0.0.1:3080\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: aGVsbG8=\r\nSec-WebSocket-Version: 13\r\n\r\n")
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

#[tokio::test]
async fn unreachable_target_yields_502_with_reason() {
    let proxy_addr = spawn_proxy(Arc::new(DeadDialer), 3080).await;
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

