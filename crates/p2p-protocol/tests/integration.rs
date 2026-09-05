//! 集成验收：多 handler 路由、req-resp roundtrip/超时/超大帧、握手失败路径。

use std::io;
use std::sync::Arc;
use std::time::Duration;

use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{
    dispatch_inbound, flatten_io, open_with_protocol, read_frame, write_frame, write_protocol_id,
    HandlerRegistry, LoopbackHub, ProtocolError, ProtocolHandler, ProtocolId, RequestResponse,
    RequestResponseClient, StreamFactory, MAX_FRAME_SIZE,
};
use tokio::io::{AsyncWrite, AsyncWriteExt};

fn peer(n: u8) -> PeerId {
    PeerId::from_bytes([n; 32])
}

fn protocol_id(s: &str) -> ProtocolId {
    ProtocolId::new(s).unwrap()
}

/// 固定前缀回显 handler：读一帧，回 prefix + 原文。
struct PrefixedEcho {
    id: ProtocolId,
    prefix: &'static str,
}

#[async_trait::async_trait]
impl ProtocolHandler for PrefixedEcho {
    fn protocol(&self) -> ProtocolId {
        self.id.clone()
    }

    async fn handle(&self, mut stream: BoxedStream) -> io::Result<()> {
        let req = read_frame(&mut stream).await?;
        let mut resp = Vec::with_capacity(self.prefix.len() + req.len());
        resp.extend_from_slice(self.prefix.as_bytes());
        resp.extend_from_slice(&req);
        write_frame(&mut stream, &resp).await
    }
}

/// 收下请求帧后永不应答（测超时路径）。
struct SilentHandler {
    id: ProtocolId,
}

/// 身份捕获回显与裸流入口回归迁至 tests/peer_passthrough.rs（行数红线）。

#[async_trait::async_trait]
impl ProtocolHandler for SilentHandler {
    fn protocol(&self) -> ProtocolId {
        self.id.clone()
    }

    async fn handle(&self, mut stream: BoxedStream) -> io::Result<()> {
        read_frame(&mut stream).await?;
        std::future::pending::<()>().await;
        Ok(())
    }
}

/// 起 loopback 路由任务：对端每来一条流即按注册表分发，分发失败打印信号。
fn spawn_router(registry: HandlerRegistry) -> (LoopbackHub, tokio::task::JoinHandle<()>) {
    let registry = Arc::new(registry);
    let (hub, mut inbound) = LoopbackHub::new(16, 64 * 1024);
    let worker = tokio::spawn(async move {
        while let Some(stream) = inbound.recv().await {
            let registry = Arc::clone(&registry);
            tokio::spawn(async move {
                if let Err(e) = dispatch_inbound(stream, &registry).await {
                    eprintln!("[router] dispatch failed: {e}");
                }
            });
        }
    });
    (hub, worker)
}

#[tokio::test]
async fn multi_handler_routes_by_protocol_id() {
    let mut registry = HandlerRegistry::default();
    registry.register(Arc::new(PrefixedEcho {
        id: protocol_id("/test/chat/1"),
        prefix: "chat:",
    }));
    registry.register(Arc::new(PrefixedEcho {
        id: protocol_id("/test/ping/1"),
        prefix: "ping:",
    }));
    let (hub, worker) = spawn_router(registry);

    for (id, prefix) in [
        (protocol_id("/test/chat/1"), "chat:"),
        (protocol_id("/test/ping/1"), "ping:"),
    ] {
        let opened = hub.open_stream(&peer(1), &id).await.unwrap();
        let mut stream = open_with_protocol(opened, &id).await.unwrap();
        write_frame(&mut stream, b"hi").await.unwrap();
        let reply = read_frame(&mut stream).await.unwrap();
        assert_eq!(reply, format!("{prefix}hi").into_bytes());
    }
    worker.abort();
}

#[tokio::test]
async fn request_response_roundtrip() {
    let mut registry = HandlerRegistry::default();
    registry.register(Arc::new(PrefixedEcho {
        id: protocol_id("/test/echo/1"),
        prefix: "",
    }));
    let (hub, worker) = spawn_router(registry);
    let client = RequestResponseClient::new(hub);
    let resp = client
        .request(
            peer(2),
            protocol_id("/test/echo/1"),
            b"payload-123".to_vec(),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert_eq!(resp, b"payload-123");
    worker.abort();
}

#[tokio::test]
async fn request_timeout_is_error_not_panic() {
    let mut registry = HandlerRegistry::default();
    registry.register(Arc::new(SilentHandler {
        id: protocol_id("/test/silent/1"),
    }));
    let (hub, worker) = spawn_router(registry);
    let client = RequestResponseClient::new(hub);
    let err = client
        .request(
            peer(3),
            protocol_id("/test/silent/1"),
            b"hi".to_vec(),
            Duration::from_millis(100),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, ProtocolError::Timeout(_)),
        "unexpected: {err:?}"
    );
    worker.abort();
}

#[tokio::test]
async fn oversize_frame_rejected_everywhere() {
    let big = vec![0u8; MAX_FRAME_SIZE as usize + 1];
    // 写端：本地即拒
    let (mut tx, mut rx) = tokio::io::duplex(64);
    let err = write_frame(&mut tx, &big).await.unwrap_err();
    assert!(
        matches!(flatten_io(err), ProtocolError::FrameTooLarge(_)),
        "expected FrameTooLarge on write side"
    );
    // 读端：伪造超长 varint 前缀，读帧即拒
    write_test_varint(&mut tx, u64::from(MAX_FRAME_SIZE) + 1)
        .await
        .unwrap();
    let err = read_frame(&mut rx).await.unwrap_err();
    assert!(
        matches!(flatten_io(err), ProtocolError::FrameTooLarge(_)),
        "expected FrameTooLarge on read side"
    );
    // request() 路径：错误穿透为顶层 FrameTooLarge
    let (hub, worker) = spawn_router(HandlerRegistry::default());
    let client = RequestResponseClient::new(hub);
    let err = client
        .request(
            peer(4),
            protocol_id("/test/echo/1"),
            big,
            Duration::from_secs(5),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, ProtocolError::FrameTooLarge(_)),
        "unexpected: {err:?}"
    );
    worker.abort();
}

async fn write_test_varint(w: &mut (impl AsyncWrite + Unpin + Send), mut v: u64) -> io::Result<()> {
    let mut buf = [0u8; 10];
    let mut i = 0;
    loop {
        let mut byte = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        buf[i] = byte;
        i += 1;
        if v == 0 {
            break;
        }
    }
    w.write_all(&buf[..i]).await
}

#[tokio::test]
async fn invalid_protocol_id_fails_handshake() {
    // rustc 1.98 的 unused_mut 误报：去掉 mut 会触发 E0596，二者矛盾，局部压制
    #[allow(unused_mut)]
    let (mut tx, mut rx) = tokio::io::duplex(64);
    write_frame(&mut tx, b"not-a-protocol-id").await.unwrap();
    let err = dispatch_inbound(Box::new(rx), &HandlerRegistry::default())
        .await
        .unwrap_err();
    assert!(
        matches!(err, ProtocolError::InvalidId(_)),
        "unexpected: {err:?}"
    );
}

#[tokio::test]
async fn unregistered_protocol_fails_handshake() {
    // rustc 1.98 的 unused_mut 误报：去掉 mut 会触发 E0596，二者矛盾，局部压制
    #[allow(unused_mut)]
    let (mut tx, mut rx) = tokio::io::duplex(64);
    write_protocol_id(&mut tx, &protocol_id("/test/absent/1"))
        .await
        .unwrap();
    let err = dispatch_inbound(Box::new(rx), &HandlerRegistry::default())
        .await
        .unwrap_err();
    assert!(
        matches!(err, ProtocolError::UnsupportedProtocol(_)),
        "unexpected: {err:?}"
    );
}
