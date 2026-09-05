//! wire 线协议单测（wire.rs 行数红线拆分）：fromAddrs 加法字段 serde 兼容。

use crate::model::{ChatEnvelope, ChatKind, ChatStatus, Sender};
use crate::wire::WireEnvelope;

fn env() -> ChatEnvelope {
    ChatEnvelope {
        id: "m1".into(),
        peer: "p".into(),
        sender: Sender::Me,
        kind: ChatKind::Text,
        ts_ms: 1,
        text: Some("hi".into()),
        media: None,
        status: ChatStatus::Pending,
        reply_to: None,
    }
}

#[test]
fn from_addrs_roundtrip_and_legacy_json_compat() {
    let local = crate::model::parse_peer_id(&"1".repeat(32)).unwrap();
    let wire = WireEnvelope::from_outbound(&env(), local, vec!["127.0.0.1/u1".into()]);
    let json = serde_json::to_string(&wire).unwrap();
    assert!(json.contains("fromAddrs"), "camelCase 字段名");
    let back: WireEnvelope = serde_json::from_str(&json).unwrap();
    assert_eq!(back.from_addrs, Some(vec!["127.0.0.1/u1".to_string()]));
    // 旧对端缺字段可读（serde default）
    let legacy: WireEnvelope =
        serde_json::from_str(&json.replace(r#","fromAddrs":["127.0.0.1/u1"]"#, "")).unwrap();
    assert_eq!(legacy.from_addrs, None);
    // 空声明地址不上 wire（一次性进程不污染对端好友簿）
    let bare = WireEnvelope::from_outbound(&env(), local, Vec::new());
    assert_eq!(bare.from_addrs, None);
}
