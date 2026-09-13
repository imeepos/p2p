//! 入站接线级测试（Amended A-1 红绿锚点）：Deny = 不落库、不广播事件、不回任何帧；
//! Allow = ACK + 落库 + 事件（现状行为不变）；send 闸放行而 attachment 闸拒时，
//! 文本消息入列、媒体消息整帧拒——判定次序在 handler 层可观测。

use std::path::PathBuf;
use std::sync::Arc;

use p2p::ProtocolHandler;
use p2p::ProtocolId;
use p2p_identity::PeerId;
use p2p_protocol::write_frame;
use tokio::io::DuplexStream;

use crate::events::ChatEvent;
use crate::gate::{Gate, Reject};
use crate::wire::{self, AckFrame, ChatHandler, ENVELOPE};
use crate::ChatCore;

/// 可编程假闸：deny_at=Some(false) 拒 send 闸、Some(true) 拒 attachment 闸、None 恒 Allow。
struct FixedGate {
    deny_at: Option<bool>,
}

impl crate::gate::CheckGate for FixedGate {
    fn admit(&self, _peer: &PeerId, media: bool) -> Result<(), Reject> {
        if self.deny_at == Some(media) {
            return Err(Reject {
                reason: "NotBound".into(),
            });
        }
        Ok(())
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-chat-wiregate-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

async fn assembled(tag: &str, gate: Gate) -> (Arc<ChatCore>, PathBuf) {
    let dir = temp_dir(tag);
    let node = Arc::new(
        p2p::Node::builder()
            .mdns(false)
            .data_dir(dir.join("node"))
            .build()
            .await
            .expect("test node"),
    );
    let chat = crate::Chat::with_gate(
        node,
        dir.clone(),
        Arc::new(crate::PeerProfile::default),
        gate,
    )
    .expect("chat assembled");
    (chat.core.clone(), dir)
}

/// 远端视角入站信封帧（sender=me、peer=远端自身 id、camelCase 与线上形状一致）。
fn wire_env(remote: PeerId, id: &str, media: Option<(&str, &str)>) -> Vec<u8> {
    let payload = serde_json::json!({
        "id": id,
        "peer": remote.to_string(),
        "sender": "me",
        "kind": if media.is_some() { "image" } else { "text" },
        "tsMs": 1,
        "text": if media.is_none() { serde_json::json!("hi") } else { serde_json::Value::Null },
        "media": media
            .map(|(name, mime)| serde_json::json!({"name": name, "mime": mime, "size": 4u64}))
            .unwrap_or(serde_json::Value::Null),
        "replyTo": serde_json::Value::Null,
    });
    let bytes = serde_json::to_vec(&payload).expect("serialize");
    let mut frame = Vec::with_capacity(1 + bytes.len());
    frame.push(ENVELOPE);
    frame.extend_from_slice(&bytes);
    frame
}

/// 单次入站投递：写信封帧 → handler 处理 → 读对端回帧；None = 对端未回任何帧即断流。
async fn inbound_once(
    core: Arc<ChatCore>,
    frame: Vec<u8>,
) -> Result<Option<AckFrame>, std::io::Error> {
    let (mut mine, theirs): (DuplexStream, DuplexStream) = tokio::io::duplex(4096);
    let proto = ProtocolId::new(crate::CHAT_PROTOCOL).expect("proto id");
    let handler = ChatHandler::new(core, proto);
    let task = tokio::spawn(async move { handler.handle(Box::new(theirs)).await });
    write_frame(&mut mine, &frame).await?;
    // 读错误（含对端断流 EOF）一律归一为 None = 对端未回任何帧。
    let reply = wire::read_ack(&mut mine).await.ok();
    drop(mine);
    let _ = task.await.expect("handler task");
    Ok(reply)
}

/// send 闸拒与 attachment 闸拒两条路径：整帧拒收——不落库、不广播、不回帧。
#[tokio::test]
async fn deny_rejects_whole_frame_without_store_event_or_reply() {
    for deny_at in [Some(false), Some(true)] {
        let (core, dir) = assembled("deny", Some(Arc::new(FixedGate { deny_at }))).await;
        let mut rx = core.events.subscribe();
        let remote = PeerId::from_bytes([9u8; 32]);
        let reply = inbound_once(
            core.clone(),
            wire_env(remote, "m1", Some(("a.png", "image/png"))),
        )
        .await
        .expect("handler io");
        assert!(reply.is_none(), "拒收不得回任何帧");
        assert!(
            !core.store.has_message(&remote.to_string(), "m1"),
            "拒收不得落库"
        );
        assert!(rx.try_recv().is_err(), "拒收不得广播 chat_message 事件");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[tokio::test]
async fn allow_still_acks_stores_and_broadcasts() {
    let (core, dir) = assembled("allow", Some(Arc::new(FixedGate { deny_at: None }))).await;
    let mut rx = core.events.subscribe();
    let remote = PeerId::from_bytes([8u8; 32]);
    let ack = inbound_once(core.clone(), wire_env(remote, "m2", None))
        .await
        .expect("ack read")
        .expect("Allow 必须回 ACK");
    assert!(ack.ok && ack.id == "m2");
    assert!(
        core.store.has_message(&remote.to_string(), "m2"),
        "Allow 落库"
    );
    match rx.try_recv().expect("Allow 广播事件") {
        ChatEvent::ChatMessage { peer, message } => {
            assert_eq!(peer, remote.to_string());
            assert_eq!(message.id, "m2");
        }
        other => panic!("意外事件: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// 判定次序 handler 级证据：send 闸放行（文本入列）而 attachment 闸拒（媒体整帧拒）。
#[tokio::test]
async fn attachment_gate_only_blocks_media_messages() {
    let (core, dir) = assembled(
        "order",
        Some(Arc::new(FixedGate {
            deny_at: Some(true),
        })),
    )
    .await;
    let remote = PeerId::from_bytes([7u8; 32]);
    inbound_once(core.clone(), wire_env(remote, "t1", None))
        .await
        .expect("text ack")
        .expect("send 闸放行的文本必须回 ACK");
    assert!(core.store.has_message(&remote.to_string(), "t1"));
    let reply = inbound_once(
        core.clone(),
        wire_env(remote, "t2", Some(("a.png", "image/png"))),
    )
    .await
    .expect("handler io");
    assert!(reply.is_none(), "attachment 闸拒必须整帧拒收");
    assert!(!core.store.has_message(&remote.to_string(), "t2"));
    let _ = std::fs::remove_dir_all(&dir);
}
