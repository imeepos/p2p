//! 反代行为单测（迁移自 apps/gui/src-tauri/src/tunnel/proxy_tests.rs，用例
//! 一条不减）：真实回环 TCP 对端验证 Host 重写、流式转发、升级裸泵与拒绝
//! 路径。wire 侧为帧面（pump/ManualOpener 消费帧），对端 helper 统一
//! 「隧道皮」（读协议 ID/票据、回 ack）后按帧收发 HTTP 字节。

mod http;
mod ws;

use std::sync::Arc;

use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

use super::{LocalProxy, TunnelOpener};
use crate::{TunnelError, TunnelIo};

/// 自足握手开隧道（单测）：connect → 协议 ID 帧 → 票据帧 → 等 ack → 裸流。
/// 帧序与生产 TunnelClient 一致（严格无 skip）；因未含其内部泵，open 返回
/// 后 wire 上即裸 HTTP 字节——对端 helper 相应用裸字节收发。
struct ManualOpener {
    addr: std::net::SocketAddr,
    target: String,
}

#[async_trait::async_trait]
impl TunnelOpener for ManualOpener {
    async fn open(&self, uid: &str, _target: &str) -> Result<TunnelIo, TunnelError> {
        use p2p_protocol::{open_with_protocol, read_frame, write_frame};
        let protocol = p2p_protocol::ProtocolId::new(crate::PROTOCOL_ID)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let raw: p2p_mux::BoxedStream = Box::new(
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
        let ticket = crate::TunnelTicket::new(uid, self.target.clone(), nonce)
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
    async fn open(&self, _uid: &str, _target: &str) -> Result<TunnelIo, TunnelError> {
        Err(TunnelError::Io(std::io::Error::other("target down")))
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
        crate::PROTOCOL_ID,
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
