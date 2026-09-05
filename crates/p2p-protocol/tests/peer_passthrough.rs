//! peer_passthrough 集成验收：分发层下传的 PeerId 与拨号方身份一致（BASE1）。

use std::io;
use std::sync::Arc;

use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{
    dispatch_inbound, dispatch_inbound_with_peer, open_with_protocol, read_frame, write_frame,
    HandlerRegistry, ProtocolHandler, ProtocolId,
};

fn peer(n: u8) -> PeerId {
    PeerId::from_bytes([n; 32])
}

fn protocol_id(s: &str) -> ProtocolId {
    ProtocolId::new(s).unwrap()
}

/// 身份捕获回显：覆写 handle_inbound 记录分发层下传的对端身份；
/// 裸流入口（handle）到达即失败，证明带身份分发只走新接缝。
struct IdentityCapture {
    id: ProtocolId,
    seen: tokio::sync::mpsc::UnboundedSender<PeerId>,
}

#[async_trait::async_trait]
impl ProtocolHandler for IdentityCapture {
    fn protocol(&self) -> ProtocolId {
        self.id.clone()
    }

    async fn handle_inbound(&self, peer: PeerId, mut stream: BoxedStream) -> io::Result<()> {
        let _ = self.seen.send(peer);
        let req = read_frame(&mut stream).await?;
        write_frame(&mut stream, &req).await
    }

    async fn handle(&self, _stream: BoxedStream) -> io::Result<()> {
        Err(io::Error::other(
            "peer-aware dispatch must use handle_inbound",
        ))
    }
}

/// peer_passthrough 回归：分发层下传的 PeerId 必须与拨号方身份一致，
/// handler 不再依赖「在线集推断」绕行（ISSUE 2026-09-05 底座契约缺口）。
#[tokio::test]
async fn peer_passthrough_handler_receives_dialer_identity() {
    let dialer = peer(0x21);
    let proto = protocol_id("/test/identity/1");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut registry = HandlerRegistry::default();
    registry.register(Arc::new(IdentityCapture {
        id: proto.clone(),
        seen: tx,
    }));

    let (client, server) = tokio::io::duplex(4096);
    let server_task = tokio::spawn(async move {
        let _ = dispatch_inbound_with_peer(Box::new(server), Some(dialer), &registry).await;
    });

    let mut stream = open_with_protocol(Box::new(client), &proto)
        .await
        .expect("protocol handshake");
    write_frame(&mut stream, b"who-am-i").await.expect("write");
    let resp = read_frame(&mut stream).await.expect("echo");
    assert_eq!(resp, b"who-am-i".to_vec());
    server_task.await.expect("server task");

    let seen = rx.recv().await.expect("handler must be invoked");
    assert_eq!(seen, dialer, "handler must receive the dialer identity");
}

/// 裸流入口（无身份上下文）仍走旧签名 handle：盲拨应答场景行为不变。
#[tokio::test]
async fn bare_dispatch_keeps_legacy_handle_entry() {
    let proto = protocol_id("/test/identity/1");
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let mut registry = HandlerRegistry::default();
    registry.register(Arc::new(IdentityCapture {
        id: proto.clone(),
        seen: tx,
    }));

    let (client, server) = tokio::io::duplex(4096);
    let server_task = tokio::spawn(async move {
        let served = dispatch_inbound(Box::new(server), &registry).await;
        assert!(served.is_err(), "bare dispatch must reach legacy handle");
    });
    let _ = open_with_protocol(Box::new(client), &proto).await;
    server_task.await.expect("server task");
}
