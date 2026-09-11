//! responder 单元测试（经 `#[path]` 挂回 responder.rs，文件名约定豁免 panic 扫描）。
use super::*;
use std::collections::HashSet;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct EchoDialer;

#[async_trait]
impl HttpDialer for EchoDialer {
    async fn dial(&self, _target: &str) -> Result<TunnelIo, TunnelError> {
        let (a, b) = tokio::io::duplex(4096);
        tokio::spawn(async move {
            // echo：读半进写半出；EOF 后写半关（半关透传）
            let (mut r, mut w) = tokio::io::split(b);
            let _ = tokio::io::copy(&mut r, &mut w).await;
            let _ = w.shutdown().await;
        });
        Ok(Box::new(a))
    }
}

fn ticket() -> TunnelTicket {
    TunnelTicket::new("0011223344556677", "127.0.0.1:8014", "ab".repeat(16)).unwrap()
}

fn gate_open() -> TunnelGate {
    TunnelGate::new(TunnelServeConfig {
        allowlist: HashSet::from(["127.0.0.1:8014".to_string()]),
        ..Default::default()
    })
}

async fn serve(responder: Arc<TunnelResponder<EchoDialer>>) -> tokio::io::DuplexStream {
    let (visitor, inbound) = tokio::io::duplex(64 * 1024);
    tokio::spawn(async move {
        let _ = responder
            .serve_session(PeerId::from_bytes([7; 32]), Box::new(inbound))
            .await;
    });
    visitor
}

async fn reply(visitor: &mut tokio::io::DuplexStream) -> TunnelReply {
    write_frame(visitor, &ticket().encode().unwrap())
        .await
        .unwrap();
    TunnelReply::decode(&read_frame(visitor).await.unwrap()).unwrap()
}

fn open_responder(gate: TunnelGate) -> Arc<TunnelResponder<EchoDialer>> {
    Arc::new(TunnelResponder::new(
        crate::protocol_id().unwrap(),
        gate,
        EchoDialer,
    ))
}

#[tokio::test]
async fn ack_then_bidirectional_echo_and_audit() {
    let responder = open_responder({
        let gate = gate_open();
        gate.set_enabled(true);
        gate
    });
    let mut visitor = serve(responder.clone()).await;
    assert_eq!(
        reply(&mut visitor).await,
        TunnelReply::ack("0011223344556677")
    );
    write_frame(&mut visitor, b"ping").await.unwrap();
    assert_eq!(read_frame(&mut visitor).await.unwrap(), b"ping");
    visitor.shutdown().await.unwrap();
    let mut rest = Vec::new();
    visitor.read_to_end(&mut rest).await.unwrap();
    for _ in 0..100 {
        if !responder.audit().snapshot().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let records = responder.audit().snapshot();
    assert_eq!(records.len(), 1, "{records:?}");
    assert_eq!(records[0].outcome, TunnelAuditOutcome::Served);
    assert_eq!(records[0].target, "127.0.0.1:8014");
    assert_eq!(records[0].peer_id, PeerId::from_bytes([7; 32]).to_string());
}

#[tokio::test]
async fn rejection_paths_send_explicit_error_frames() {
    // 按次开关关闭
    let responder = open_responder(gate_open());
    let mut visitor = serve(responder).await;
    assert_eq!(
        reply(&mut visitor).await,
        TunnelReply::error(TunnelErrorCode::Shutdown, "gate: shutdown")
    );
    // 白名单未中
    let gate = gate_open();
    gate.set_enabled(true);
    let mut visitor = serve(open_responder(gate)).await;
    let off = TunnelTicket::new("0011223344556677", "127.0.0.1:9", "ab".repeat(16)).unwrap();
    write_frame(&mut visitor, &off.encode().unwrap())
        .await
        .unwrap();
    let frame = TunnelReply::decode(&read_frame(&mut visitor).await.unwrap()).unwrap();
    assert!(matches!(
        frame,
        TunnelReply::Error {
            code: TunnelErrorCode::TargetNotAllowed,
            ..
        }
    ));
    // 票据帧超 4 KiB
    let gate = gate_open();
    gate.set_enabled(true);
    let responder = open_responder(gate);
    let mut visitor = serve(responder.clone()).await;
    write_frame(&mut visitor, &vec![0u8; MAX_TICKET_BYTES + 1])
        .await
        .unwrap();
    let frame = TunnelReply::decode(&read_frame(&mut visitor).await.unwrap()).unwrap();
    assert!(matches!(
        frame,
        TunnelReply::Error {
            code: TunnelErrorCode::BadTicket,
            ..
        }
    ));
    let rejected = responder.audit().snapshot();
    assert_eq!(rejected.len(), 1, "仅最后一例落此账: {rejected:?}");
    assert_eq!(
        rejected[0].outcome,
        TunnelAuditOutcome::Rejected(TunnelErrorCode::BadTicket)
    );
}
