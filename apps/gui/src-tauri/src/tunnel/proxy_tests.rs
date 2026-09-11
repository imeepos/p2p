//! 反代行为单测：真实回环 TCP 对端验证 Host 重写、流式转发、升级裸泵与
//! 拒绝路径。wire 侧为帧面（pump/ManualOpener 消费帧），对端 helper 统一
//! 「隧道皮」（读协议 ID/票据、回 ack）后按帧收发 HTTP 字节。

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::LocalProxy;
use super::TunnelOpener;

/// 自足握手开隧道（单测）：connect → 协议 ID 帧 → 票据帧 → 等 ack → 裸流。
/// 帧序与生产 TunnelClient 一致（严格无 skip）；因未含其内部泵，open 返回
/// 后 wire 上即裸 HTTP 字节——对端 helper 相应用裸字节收发。
struct ManualOpener {
    addr: std::net::SocketAddr,
    target: String,
}

#[async_trait::async_trait]
impl TunnelOpener for ManualOpener {
    async fn open(
        &self,
        uid: &str,
        _target: &str,
    ) -> Result<p2p_tunnel::TunnelIo, p2p_tunnel::TunnelError> {
        use p2p_protocol::{open_with_protocol, read_frame, write_frame};
        let protocol = p2p::ProtocolId::new(p2p_tunnel::PROTOCOL_ID)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let raw: p2p::BoxedStream = Box::new(
            TcpStream::connect(self.addr)
                .await
                .map_err(std::io::Error::other)?,
        );
        let mut stream = open_with_protocol(raw, &protocol)
            .await
            .map_err(std::io::Error::other)?;
        let mut nonce = [0u8; 16];
        getrandom::getrandom(&mut nonce).map_err(std::io::Error::other)?;
        let nonce = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let ticket = p2p_tunnel::TunnelTicket::new(uid, self.target.clone(), nonce)
            .map_err(std::io::Error::other)?;
        write_frame(
            &mut stream,
            &ticket.encode().map_err(std::io::Error::other)?,
        )
        .await
        .map_err(std::io::Error::other)?;
        let ack = read_frame(&mut stream)
            .await
            .map_err(std::io::Error::other)?;
        let ack: serde_json::Value = serde_json::from_slice(&ack).map_err(std::io::Error::other)?;
        assert_eq!(ack["uid"], uid, "ack uid 与票据不一致");
        Ok(Box::new(stream))
    }
}

/// 开隧道恒失败（隧道不可达）。
struct DeadOpener;

#[async_trait::async_trait]
impl TunnelOpener for DeadOpener {
    async fn open(
        &self,
        _uid: &str,
        _target: &str,
    ) -> Result<p2p_tunnel::TunnelIo, p2p_tunnel::TunnelError> {
        Err(p2p_tunnel::TunnelError::Io(std::io::Error::other(
            "target down",
        )))
    }
}

async fn spawn_proxy(opener: Arc<dyn TunnelOpener>, target_port: u16) -> std::net::SocketAddr {
    let proxy = LocalProxy::bind(target_port, "test-peer".to_string(), opener)
        .await
        .expect("bind");
    let addr = proxy.local_addr();
    tokio::spawn(proxy.serve());
    addr
}

/// 隧道皮：读协议 ID 帧（严格断言，双写装配检测）+ 票据帧，回 ack(uid)。
async fn tunnel_skin(conn: &mut TcpStream) {
    let proto = p2p_protocol::read_frame(conn).await.expect("proto frame");
    assert_eq!(
        std::str::from_utf8(&proto).expect("utf8"),
        p2p_tunnel::PROTOCOL_ID,
        "流上首帧必须是协议 ID（双写装配检测）"
    );
    let ticket = p2p_protocol::read_frame(conn).await.expect("ticket frame");
    let ticket: serde_json::Value = serde_json::from_slice(&ticket).expect("ticket json");
    let ack = serde_json::json!({ "k": "ack", "uid": ticket["uid"] });
    p2p_protocol::write_frame(conn, &serde_json::to_vec(&ack).expect("ack"))
        .await
        .expect("ack write");
}

/// 裸字节循环读：拼字节直到谓词满足（ManualOpener 无泵，wire=裸 HTTP）。
async fn read_bytes_until(
    conn: &mut TcpStream,
    mut buf: Vec<u8>,
    done: impl Fn(&[u8]) -> bool,
) -> Vec<u8> {
    let mut chunk = [0u8; 4096];
    while !done(&buf) {
        let read = conn.read(&mut chunk).await.expect("read");
        assert!(read > 0, "EOF before complete");
        buf.extend_from_slice(&chunk[..read]);
    }
    buf
}

fn is_complete_request(buf: &[u8]) -> bool {
    buf.windows(4).any(|w| w == b"\r\n\r\n")
}

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
