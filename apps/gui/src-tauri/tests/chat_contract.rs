//! gui-contract.md §12.3 契约 roundtrip 与校验矩阵（T32，纯 serde，不起节点）。
//!
//! 断言 p2p-chat 返回类型的序列化字段名与契约逐字一致（camelCase、Option→null），
//! 并覆盖 ChatMessageJson/ChatFriendJson/ChatSendReport 全字段 roundtrip；
//! 校验矩阵覆盖文本/媒体/昵称的边界（peerId 与 addr 走命令层冒烟测试）。

use p2p_chat::{
    sanitize_name, validate_media, validate_text, ChatEnvelope, ChatFriend, ChatKind,
    ChatMediaMeta, ChatSendReport, ChatStatus, Sender, MAX_MESSAGE_SIZE,
};
use p2p_console::types::NodeEventJson;
use serde_json::json;

fn sample_envelope(media: Option<ChatMediaMeta>) -> ChatEnvelope {
    ChatEnvelope {
        id: "id-1".into(),
        peer: "PeerA".into(),
        sender: Sender::Me,
        kind: ChatKind::Text,
        ts_ms: 1_720_000_000_000,
        text: Some("你好世界".into()),
        media,
        status: ChatStatus::Pending,
    }
}

#[test]
fn chat_friend_json_field_names_match_contract() {
    let friend = ChatFriend {
        peer_id: "PeerA".into(),
        nickname: "小友".into(),
        addrs: vec!["127.0.0.1/u3400".into(), "127.0.0.1/t3401".into()],
        note: None,
    };
    let encoded = serde_json::to_value(&friend).expect("序列化好友");
    assert_eq!(
        encoded,
        json!({
            "peerId": "PeerA",
            "nickname": "小友",
            "addrs": ["127.0.0.1/u3400", "127.0.0.1/t3401"],
            "note": null,
        }),
        "ChatFriendJson 字段名/空 Option 须逐字对齐 §12.3"
    );
    let decoded: ChatFriend = serde_json::from_value(encoded).expect("反序列化好友");
    assert_eq!(decoded, friend, "ChatFriendJson roundtrip 不保真");
}

#[test]
fn chat_friend_json_note_roundtrips_both_states() {
    let with_note = ChatFriend {
        peer_id: "PeerB".into(),
        nickname: "带备注".into(),
        addrs: Vec::new(),
        note: Some("同事".into()),
    };
    let value = serde_json::to_value(&with_note).expect("序列化");
    assert_eq!(value["note"], "同事");
    let back: ChatFriend = serde_json::from_value(value).expect("反序列化");
    assert_eq!(back, with_note);

    let none: ChatFriend =
        serde_json::from_value(json!({"peerId":"P","nickname":"n","addrs":[],"note":null}))
            .expect("note null 可反序列化");
    assert_eq!(none.note, None);
}

#[test]
fn chat_message_json_text_field_names_match_contract() {
    let env = sample_envelope(None);
    let encoded = serde_json::to_value(&env).expect("序列化消息");
    assert_eq!(
        encoded,
        json!({
            "id": "id-1",
            "peer": "PeerA",
            "sender": "me",
            "kind": "text",
            "tsMs": 1_720_000_000_000_i64,
            "text": "你好世界",
            "media": null,
            "status": "pending",
        }),
        "ChatMessageJson 字段名/空 Option 须逐字对齐 §12.3"
    );
    let decoded: ChatEnvelope = serde_json::from_value(encoded).expect("反序列化消息");
    assert_eq!(decoded, env, "ChatMessageJson roundtrip 不保真");
}

#[test]
fn chat_message_json_media_roundtrips_with_path_null() {
    let env = sample_envelope(Some(ChatMediaMeta {
        name: "photo.png".into(),
        mime: "image/png".into(),
        size: 4,
        path: Some("/data/chat/media/PeerA/id-1_photo.png".into()),
    }));
    let value = serde_json::to_value(&env).expect("序列化媒体消息");
    assert_eq!(
        value["media"],
        json!({
            "name": "photo.png",
            "mime": "image/png",
            "size": 4,
            "path": "/data/chat/media/PeerA/id-1_photo.png",
        })
    );
    let back: ChatEnvelope = serde_json::from_value(value).expect("反序列化");
    assert_eq!(back, env);

    let path_none: ChatEnvelope = serde_json::from_value(json!({
        "id": "id-2",
        "peer": "PeerA",
        "sender": "them",
        "kind": "image",
        "tsMs": 0,
        "text": null,
        "media": {"name": "a.png", "mime": "image/png", "size": 3, "path": null},
        "status": "delivered",
    }))
    .expect("path null 可反序列化");
    assert_eq!(path_none.media.expect("必有媒体").path, None);
    assert_eq!(path_none.sender, Sender::Them);
}

#[test]
fn chat_send_report_roundtrips() {
    let report = ChatSendReport {
        message: sample_envelope(None),
        delivered: true,
    };
    let encoded = serde_json::to_value(&report).expect("序列化发送报告");
    assert_eq!(
        encoded,
        json!({
            "message": {
                "id": "id-1",
                "peer": "PeerA",
                "sender": "me",
                "kind": "text",
                "tsMs": 1_720_000_000_000_i64,
                "text": "你好世界",
                "media": null,
                "status": "pending",
            },
            "delivered": true,
        }),
        "ChatSendReport 字段名须逐字对齐 §12.3"
    );
    let decoded: ChatSendReport = serde_json::from_value(encoded).expect("反序列化");
    assert_eq!(decoded, report, "ChatSendReport roundtrip 不保真");
}

#[test]
fn chat_event_json_shapes_match_contract() {
    let message = sample_envelope(None);
    let msg_event = NodeEventJson::ChatMessage {
        peer: "PeerA".into(),
        message: message.clone(),
        ts_ms: None,
    };
    let value = serde_json::to_value(&msg_event).expect("序列化 chat_message");
    assert_eq!(
        value,
        json!({
            "type": "chat_message",
            "peer": "PeerA",
            "message": serde_json::to_value(&message).unwrap(),
        }),
        "chat_message 事件形状须对齐 §12.2（tsMs 缺省不出现）"
    );

    let status_event = NodeEventJson::ChatStatus {
        peer: "PeerA".into(),
        message_id: "id-1".into(),
        status: ChatStatus::Delivered,
        ts_ms: None,
    };
    let value = serde_json::to_value(&status_event).expect("序列化 chat_status");
    assert_eq!(
        value,
        json!({
            "type": "chat_status",
            "peer": "PeerA",
            "messageId": "id-1",
            "status": "delivered",
        }),
        "chat_status 事件形状须对齐 §12.2"
    );
}

#[test]
fn chat_event_stamped_adds_tsms() {
    let stamped = NodeEventJson::ChatStatus {
        peer: "P".into(),
        message_id: "m".into(),
        status: ChatStatus::Sent,
        ts_ms: None,
    }
    .stamped(1234);
    let value = serde_json::to_value(stamped).expect("序列化");
    assert_eq!(value["tsMs"], 1234, "盖戳后 tsMs 必现");
    assert_eq!(value["type"], "chat_status");
}

#[test]
fn from_chat_event_maps_both_variants() {
    let message = sample_envelope(None);
    let mapped: NodeEventJson = p2p_chat::ChatEvent::ChatMessage {
        peer: "PeerA".into(),
        message: message.clone(),
    }
    .into();
    assert_eq!(
        mapped,
        NodeEventJson::ChatMessage {
            peer: "PeerA".into(),
            message,
            ts_ms: None,
        }
    );

    // 带媒体消息：输出边界把落盘绝对路径转成 asset URL（前端 MediaContent 接缝）
    let media_msg = sample_envelope(Some(ChatMediaMeta {
        name: "photo.png".into(),
        mime: "image/png".into(),
        size: 4,
        path: Some("/data/chat/media/PeerA/id-1_photo.png".into()),
    }));
    let mapped: NodeEventJson = p2p_chat::ChatEvent::ChatMessage {
        peer: "PeerA".into(),
        message: media_msg,
    }
    .into();
    match mapped {
        NodeEventJson::ChatMessage { message, .. } => {
            let path = message.media.expect("媒体仍在").path.expect("path 已转换");
            assert!(
                path.starts_with("asset://localhost/"),
                "事件媒体 path 应为 asset URL: {path}"
            );
            assert!(path.contains("photo.png"), "文件名保留: {path}");
        }
        other => panic!("期望 chat_message，实得 {other:?}"),
    }

    let mapped: NodeEventJson = p2p_chat::ChatEvent::ChatStatus {
        peer: "PeerA".into(),
        message_id: "m-9".into(),
        status: ChatStatus::Failed,
    }
    .into();
    assert_eq!(
        mapped,
        NodeEventJson::ChatStatus {
            peer: "PeerA".into(),
            message_id: "m-9".into(),
            status: ChatStatus::Failed,
            ts_ms: None,
        }
    );
}

// ---- 校验矩阵（命令底层同一校验，可读中文 Err 断言） ----

#[test]
fn text_validation_matrix_readable_chinese() {
    let empty = validate_text("   ").expect_err("空白文本必须拒绝");
    assert!(empty.to_string().contains("文本为空"), "实际: {empty}");

    let over = validate_text(&"a".repeat(2001)).expect_err("超长文本必须拒绝");
    assert!(over.to_string().contains("上限"), "实际: {over}");

    assert_eq!(validate_text("  hello  ").expect("trim 后合法"), "hello");
}

#[test]
fn media_validation_matrix_readable_chinese() {
    let mismatch =
        validate_media(&ChatKind::Image, "text/plain", 4).expect_err("mime 与 kind 不匹配必须拒绝");
    assert!(mismatch.to_string().contains("MIME"), "实际: {mismatch}");

    let over = validate_media(&ChatKind::Image, "image/png", MAX_MESSAGE_SIZE + 1)
        .expect_err("超 64MiB 必须拒绝");
    assert!(over.to_string().contains("64MiB"), "实际: {over}");

    let empty = validate_media(&ChatKind::File, "application/octet-stream", 0)
        .expect_err("零字节附件必须拒绝");
    assert!(empty.to_string().contains("为空"), "实际: {empty}");

    assert!(validate_media(&ChatKind::Image, "image/png", 1).is_ok());
    assert!(validate_media(&ChatKind::File, "application/octet-stream", 1).is_ok());
}

#[test]
fn sanitize_name_keeps_ascii_safe() {
    assert_eq!(sanitize_name("../../etc/passwd"), "....etcpasswd");
    assert_eq!(sanitize_name(""), "attachment");
}
