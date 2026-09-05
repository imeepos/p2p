//! gui-contract.md §7 群契约 roundtrip 与事件映射（G4，纯 serde，不起节点）。
//!
//! 断言 p2p-chat 群类型序列化字段名与契约逐字一致（camelCase、Option→null、
//! acks 缺省容忍旧记录）；事件映射断言 GroupEvent → NodeEventJson 的 type 标签、
//! groupId/messageId 字段名与媒体 asset URL 输出边界转换。

use p2p_chat::{
    ChatKind, ChatMediaMeta, ChatStatus, GroupEvent, GroupInfo, GroupMessage, GroupSendReport,
    GroupState,
};
use p2p_console::types::NodeEventJson;
use serde_json::json;

fn sample_group(state: GroupState) -> GroupInfo {
    GroupInfo {
        group_id: "g-1".into(),
        name: "项目群".into(),
        owner: "PeerA".into(),
        members: vec!["PeerA".into(), "PeerB".into()],
        rev: 3,
        state,
        ts_ms: 1_720_000_000_000,
    }
}

fn sample_message() -> GroupMessage {
    GroupMessage {
        id: "m-1".into(),
        group_id: "g-1".into(),
        sender_id: "PeerB".into(),
        kind: ChatKind::Text,
        ts_ms: 1_720_000_000_001,
        text: Some("大家好".into()),
        media: None,
        status: ChatStatus::Delivered,
        acks: vec!["PeerA".into()],
        reply_to: None,
    }
}

#[test]
fn group_info_json_field_names_match_contract() {
    let encoded = serde_json::to_value(&sample_group(GroupState::Active)).expect("序列化群");
    assert_eq!(
        encoded,
        json!({
            "groupId": "g-1", "name": "项目群", "owner": "PeerA",
            "members": ["PeerA", "PeerB"], "rev": 3, "state": "active",
            "tsMs": 1_720_000_000_000u64
        }),
        "字段名逐字对齐 §7 GroupJson"
    );
    // 四态序列化逐字对齐
    assert_eq!(
        serde_json::to_value(&sample_group(GroupState::Left)).expect("left")["state"],
        json!("left")
    );
    assert_eq!(
        serde_json::to_value(&sample_group(GroupState::Kicked)).expect("kicked")["state"],
        json!("kicked")
    );
    assert_eq!(
        serde_json::to_value(&sample_group(GroupState::Disbanded)).expect("disbanded")["state"],
        json!("disbanded")
    );
}

#[test]
fn group_message_json_matches_contract_and_tolerates_legacy() {
    let media = ChatMediaMeta {
        name: "a.png".into(),
        mime: "image/png".into(),
        size: 4,
        path: None,
    };
    let mut msg = sample_message();
    msg.kind = ChatKind::Image;
    msg.text = None;
    msg.media = Some(media);
    let encoded = serde_json::to_value(&msg).expect("序列化消息");
    assert_eq!(encoded["groupId"], json!("g-1"), "字段逐字为 groupId");
    assert_eq!(encoded["senderId"], json!("PeerB"), "字段逐字为 senderId");
    assert_eq!(
        encoded["tsMs"],
        json!(1_720_000_000_001u64),
        "字段逐字为 tsMs"
    );
    assert_eq!(encoded["acks"], json!(["PeerA"]));
    assert_eq!(encoded["media"]["size"], json!(4));

    // 旧记录（无 acks 字段）读回 = 空数组
    let legacy = json!({
        "id": "m-0", "groupId": "g-1", "senderId": "PeerB", "kind": "text",
        "tsMs": 1, "text": "旧", "media": null, "status": "delivered", "replyTo": null
    });
    let parsed: GroupMessage = serde_json::from_value(legacy).expect("旧记录必须可读");
    assert!(parsed.acks.is_empty(), "缺 acks 读回空");
}

#[test]
fn group_send_report_json_matches_contract() {
    let report = GroupSendReport {
        message: sample_message(),
        acked: 1,
        recipients: 2,
        delivered: false,
    };
    let encoded = serde_json::to_value(&report).expect("序列化报告");
    assert_eq!(encoded["acked"], json!(1));
    assert_eq!(encoded["recipients"], json!(2));
    assert_eq!(encoded["delivered"], json!(false));
    assert_eq!(encoded["message"]["id"], json!("m-1"));
}

#[test]
fn group_events_serialize_with_contract_tags_and_camel_fields() {
    // chat_group_message
    let ev = serde_json::to_value(&GroupEvent::Message {
        group_id: "g-1".into(),
        message: sample_message(),
    })
    .expect("序列化消息事件");
    assert_eq!(ev["type"], json!("chat_group_message"));
    assert_eq!(ev["groupId"], json!("g-1"));
    assert_eq!(ev["message"]["senderId"], json!("PeerB"));

    // chat_group_status
    let ev = serde_json::to_value(&GroupEvent::Status {
        group_id: "g-1".into(),
        message_id: "m-1".into(),
        acks: vec!["PeerA".into()],
        status: ChatStatus::Delivered,
    })
    .expect("序列化状态事件");
    assert_eq!(ev["type"], json!("chat_group_status"));
    assert_eq!(ev["groupId"], json!("g-1"));
    assert_eq!(ev["messageId"], json!("m-1"));
    assert_eq!(ev["acks"], json!(["PeerA"]));
    assert_eq!(ev["status"], json!("delivered"));

    // chat_group_state
    let ev = serde_json::to_value(&GroupEvent::State {
        group: sample_group(GroupState::Active),
    })
    .expect("序列化状态事件");
    assert_eq!(ev["type"], json!("chat_group_state"));
    assert_eq!(ev["group"]["groupId"], json!("g-1"));
}

/// GroupEvent → NodeEventJson 映射：type 标签保留、媒体路径转 asset URL、盖戳生效。
#[test]
fn group_event_maps_to_node_event_json_with_asset_path_and_stamp() {
    let mut msg = sample_message();
    msg.media = Some(ChatMediaMeta {
        name: "a.png".into(),
        mime: "image/png".into(),
        size: 4,
        path: Some("/data/chat/media/g-1/m-1_a.png".into()),
    });
    let mapped = NodeEventJson::from(GroupEvent::Message {
        group_id: msg.group_id.clone(),
        message: msg.clone(),
    });
    let encoded = serde_json::to_value(&mapped).expect("序列化桥接事件");
    assert_eq!(encoded["type"], json!("chat_group_message"));
    let path = encoded["message"]["media"]["path"].as_str().expect("path");
    assert!(
        path.starts_with("asset://localhost/"),
        "输出边界转 asset: {path}"
    );
    let stamped = mapped.stamped(1_777);
    match stamped {
        NodeEventJson::ChatGroupMessage { ts_ms, .. } => assert_eq!(ts_ms, Some(1_777)),
        other => panic!("期望群消息事件，实得 {other:?}"),
    }

    let mapped = NodeEventJson::from(GroupEvent::State {
        group: sample_group(GroupState::Active),
    });
    assert!(matches!(mapped, NodeEventJson::ChatGroupState { .. }));
}
