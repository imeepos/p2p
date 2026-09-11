//! 隧道访侧客户端（冻结契约 §1/§3/§7）：对被访 peer 开流 → 首帧票据 →
//! 一帧应答（ack/error）→ 之后该流即裸字节面（HTTP 反代直读直写）。
//! 首帧应答 5s 超时（与被访侧票据超时对称）；拒绝必须显式 error 帧。
//! 注意：W-T2 的 `p2p-tunnel::TunnelClient` 落地后，本文件整体切换为消费其
//! 导出（rebase 对齐点，wire 语义两边同源于冻结契约）。

use std::time::Duration;

use p2p::{BoxedStream, Node, PeerId};
use p2p_protocol::{read_frame, write_frame};

use super::ticket::{TunnelErrorCode, TunnelTicket, PROTOCOL_ID};

/// 首帧应答护栏（冻结契约 §1 同源：5s）。
pub const FIRST_REPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// 拨流器缝：产出已完成协议 ID 握手的流（生产 = Node::new_stream）。
/// 独立成 trait 使反代单测可注入真实 TCP 对端（mock 只用于单测）。
#[async_trait::async_trait]
pub trait TunnelDialer: Send + Sync {
    async fn dial(&self) -> Result<BoxedStream, String>;
}

/// 生产拨流器：对固定 peer 开 `/p2p-base/tunnel/1` 流。
pub struct NodeDialer {
    node: std::sync::Arc<Node>,
    peer: PeerId,
}

impl NodeDialer {
    pub fn new(node: std::sync::Arc<Node>, peer: PeerId) -> Self {
        Self { node, peer }
    }
}

#[async_trait::async_trait]
impl TunnelDialer for NodeDialer {
    async fn dial(&self) -> Result<BoxedStream, String> {
        let protocol = p2p::ProtocolId::new(PROTOCOL_ID).map_err(|e| format!("protocol id: {e}"))?;
        self.node
            .new_stream(self.peer, protocol)
            .await
            .map_err(|e| format!("开流失败: {e}"))
    }
}

/// open 失败：区分被访侧显式拒绝（错误码直出）与拨号/IO 故障。
#[derive(Debug)]
pub enum OpenError {
    Rejected(TunnelErrorCode, Option<String>),
    Failed(String),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::Rejected(code, msg) => {
                write!(f, "被访侧拒绝({}): {}", code.as_str(), msg.as_deref().unwrap_or("-"))
            }
            OpenError::Failed(reason) => write!(f, "{reason}"),
        }
    }
}

/// 开隧道：票据帧 → 等一帧应答 → 返回裸字节流。`timeout` 由调用方注入
/// （生产 5s；测试注入短护栏避免慢测）。
pub async fn open_tunnel(
    dialer: &dyn TunnelDialer,
    ticket: &TunnelTicket,
    timeout: Duration,
) -> Result<BoxedStream, OpenError> {
    let bytes = ticket.encode().map_err(OpenError::Failed)?;
    let mut stream = dialer.dial().await.map_err(OpenError::Failed)?;
    write_frame(&mut stream, &bytes)
        .await
        .map_err(|e| OpenError::Failed(format!("票据帧写入失败: {e}")))?;
    let reply = tokio::time::timeout(timeout, read_frame(&mut stream))
        .await
        .map_err(|_| OpenError::Failed("应答帧超时".into()))
        .and_then(|r| r.map_err(|e| OpenError::Failed(format!("应答帧读取失败: {e}"))))?;
    verify_ack(&reply, ticket).map(|()| stream)
}

/// 应答帧校验：ack 须同 uid；error 须属闭集码。
fn verify_ack(reply: &[u8], ticket: &TunnelTicket) -> Result<(), OpenError> {
    let value: serde_json::Value =
        serde_json::from_slice(reply).map_err(|e| OpenError::Failed(format!("应答帧非 JSON: {e}")))?;
    match value.get("k").and_then(serde_json::Value::as_str) {
        Some("ack") => {
            if value.get("uid").and_then(serde_json::Value::as_str) == Some(ticket.uid.as_str()) {
                Ok(())
            } else {
                Err(OpenError::Failed("ack uid 与票据不一致".into()))
            }
        }
        Some("error") => {
            let code = value
                .get("code")
                .and_then(serde_json::Value::as_str)
                .and_then(TunnelErrorCode::from_wire)
                .ok_or_else(|| OpenError::Failed("error 帧错误码非法".into()))?;
            Err(OpenError::Rejected(
                code,
                value
                    .get("msg")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
            ))
        }
        _ => Err(OpenError::Failed("应答帧缺 k 字段".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// 环回拨流器：duplex 客户端侧 + hub 收对端侧（p2p-protocol 先例）。
    struct LoopbackDialer {
        tx: tokio::sync::mpsc::Sender<BoxedStream>,
        buf: usize,
    }

    #[async_trait::async_trait]
    impl TunnelDialer for LoopbackDialer {
        async fn dial(&self) -> Result<BoxedStream, String> {
            let (client, server) = tokio::io::duplex(self.buf);
            self.tx
                .send(Box::new(server))
                .await
                .map_err(|_| "hub closed".to_string())?;
            Ok(Box::new(client))
        }
    }

    /// 最小被访侧替身（mock 只用于单测）：读票据帧，按脚本回 ack/error/静默。
    async fn mock_responder(
        mut stream: BoxedStream,
        reply: serde_json::Value,
        delay: Option<Duration>,
    ) {
        let _ = read_frame(&mut stream).await;
        if let Some(delay) = delay {
            tokio::time::sleep(delay).await;
        }
        let frame = serde_json::to_vec(&reply).expect("reply");
        let _ = write_frame(&mut stream, &frame).await;
        let _ = stream.write_all(b"ping").await;
    }

    fn uid() -> String {
        "0123456789abcdef".into()
    }

    fn ticket() -> TunnelTicket {
        TunnelTicket {
            v: 1,
            uid: uid(),
            target: "127.0.0.1:3080".into(),
            nonce: "f".repeat(32),
            ts: 1_700_000_000,
        }
    }

    #[tokio::test]
    async fn open_sends_ticket_and_returns_stream_on_ack() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let dialer = LoopbackDialer { tx, buf: 4096 };
        let t = tokio::spawn(mock_responder(
            rx.recv().await.expect("stream"),
            serde_json::json!({"k":"ack","uid": uid()}),
            None,
        ));
        let mut stream = open_tunnel(&dialer, &ticket(), FIRST_REPLY_TIMEOUT)
            .await
            .unwrap_or_else(|e| panic!("open failed: {e}"));
        // ack 之后即裸字节面：对端写什么原样读到什么。
        t.await.expect("responder");
        let mut buf = [0u8; 4];
        stream.read_exact(&mut buf).await.expect("read");
        assert_eq!(&buf, b"ping");
    }

    #[tokio::test]
    async fn open_surveys_error_frame_as_rejected() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let dialer = LoopbackDialer { tx, buf: 4096 };
        tokio::spawn(mock_responder(
            rx.recv().await.expect("stream"),
            serde_json::json!({"k":"error","code":"target_not_allowed","msg":"nope"}),
            None,
        ));
        let err = match open_tunnel(&dialer, &ticket(), FIRST_REPLY_TIMEOUT).await {
            Err(e) => e,
            Ok(_) => panic!("expected rejection"),
        };
        assert!(matches!(
            err,
            OpenError::Rejected(TunnelErrorCode::TargetNotAllowed, Some(_))
        ));
    }

    #[tokio::test]
    async fn open_times_out_without_reply() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let dialer = LoopbackDialer { tx, buf: 4096 };
        tokio::spawn(mock_responder(
            rx.recv().await.expect("stream"),
            serde_json::json!({"k":"ack","uid": uid()}),
            Some(Duration::from_secs(10)),
        ));
        let err = match open_tunnel(&dialer, &ticket(), Duration::from_millis(50)).await {
            Err(e) => e,
            Ok(_) => panic!("expected timeout"),
        };
        assert!(err.to_string().contains("超时"));
    }

    #[tokio::test]
    async fn open_rejects_ack_with_wrong_uid() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let dialer = LoopbackDialer { tx, buf: 4096 };
        tokio::spawn(mock_responder(
            rx.recv().await.expect("stream"),
            serde_json::json!({"k":"ack","uid":"ffffffffffffffff"}),
            None,
        ));
        let err = match open_tunnel(&dialer, &ticket(), FIRST_REPLY_TIMEOUT).await {
            Err(e) => e,
            Ok(_) => panic!("expected mismatch"),
        };
        assert!(matches!(err, OpenError::Failed(_)));
    }
}
