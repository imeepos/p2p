//! 回复引用（IM-T46A）：模型 roundtrip（有/无 replyTo）、JSONL 旧记录兼容
//! （缺字段读回=无引用）、发送链路透传与非空校验。

mod common;

use common::{cleanup, peer_str, spawn};
use p2p_chat::{ChatEnvelope, ChatKind, ChatStatus, Sender};
use serde_json::json;

fn envelope_with_reply(reply_to: Option<&str>) -> ChatEnvelope {
    ChatEnvelope {
        id: "m-1".into(),
        peer: "PeerA".into(),
        sender: Sender::Me,
        kind: ChatKind::Text,
        ts_ms: 1_720_000_000_000,
        text: Some("hello".into()),
        media: None,
        status: ChatStatus::Pending,
        reply_to: reply_to.map(|s| s.to_string()),
    }
}

/// replyTo camelCase 逐字对齐；None 序列化 null；旧格式缺字段读回 = 无引用。
#[test]
fn reply_to_roundtrip_camel_case_and_missing_field_tolerance() {
    let with = envelope_with_reply(Some("quoted-1"));
    let value = serde_json::to_value(&with).expect("serialize");
    assert_eq!(value["replyTo"], "quoted-1", "replyTo 必须 camelCase");
    assert!(value.get("reply_to").is_none(), "禁止 snake_case");
    let back: ChatEnvelope = serde_json::from_value(value).expect("deserialize");
    assert_eq!(back, with, "roundtrip 不保真");

    let none = envelope_with_reply(None);
    assert_eq!(
        serde_json::to_value(&none).expect("serialize")["replyTo"],
        serde_json::Value::Null,
        "无引用序列化为 null"
    );

    // 旧格式（无 replyTo 字段，T46A 之前的历史行）缺字段读回 = 无引用
    let legacy = json!({
        "id": "legacy", "peer": "PeerA", "sender": "them", "kind": "text",
        "tsMs": 1, "text": "old", "media": null, "status": "delivered"
    });
    let parsed: ChatEnvelope = serde_json::from_value(legacy).expect("旧记录必须可读");
    assert_eq!(parsed.reply_to, None);
}

/// JSONL 旧记录兼容：无 replyTo 的历史行读回不报错，语义为无引用。
#[tokio::test]
async fn legacy_jsonl_line_reads_back_as_no_reference() {
    let a = spawn("rp-legacy").await;
    let peer_b = p2p_identity::Keypair::generate().peer_id().to_string();
    let legacy = json!({
        "id": "legacy-1", "peer": peer_b, "sender": "me", "kind": "text",
        "tsMs": 1, "text": "old-line", "media": null, "status": "pending"
    })
    .to_string();
    let path = a.dir.join("chat/messages").join(format!("{peer_b}.jsonl"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, format!("{legacy}\n")).unwrap();
    let msgs = a.chat.history(&peer_b, None, 10).expect("旧格式行必须可读");
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].id, "legacy-1");
    assert_eq!(msgs[0].reply_to, None, "缺字段读回 = 无引用");
    cleanup(&a);
}

/// 发送链路：reply_to 透传到信封与历史；空白引用可读拒绝且不落盘；None 合法。
#[tokio::test]
async fn send_passes_reply_to_through_and_rejects_blank() {
    let a = spawn("rp-send-a").await;
    let b = spawn("rp-send-b").await;
    let peer_a = peer_str(&a.node);

    // B 无 A 的地址（未加好友）→ 连接失败保持 pending，引用原样保留
    let report = b
        .chat
        .send(
            &peer_a,
            ChatKind::Text,
            Some("reply".into()),
            None,
            Some("quoted-9".into()),
        )
        .await
        .expect("send with reply");
    assert!(!report.delivered, "无地址必须不可达");
    assert_eq!(report.message.status, ChatStatus::Pending);
    assert_eq!(report.message.reply_to.as_deref(), Some("quoted-9"));
    let hist = b.chat.history(&peer_a, None, 10).expect("history");
    assert_eq!(
        hist[0].reply_to.as_deref(),
        Some("quoted-9"),
        "历史原样返回"
    );

    let err = b
        .chat
        .send(
            &peer_a,
            ChatKind::Text,
            Some("x".into()),
            None,
            Some("   ".into()),
        )
        .await
        .expect_err("空白引用必须拒绝");
    assert!(err.to_string().contains("回复引用"), "实际: {err}");
    assert_eq!(
        b.chat.history(&peer_a, None, 10).unwrap().len(),
        1,
        "被拒消息不得落盘"
    );

    // 无引用（None）合法：同为发送链路入参
    let plain = b
        .chat
        .send(&peer_a, ChatKind::Text, Some("p".into()), None, None)
        .await
        .expect("None 引用合法");
    assert_eq!(plain.message.reply_to, None);
    assert_eq!(b.chat.history(&peer_a, None, 10).unwrap().len(), 2);
    cleanup(&a);
    cleanup(&b);
}
