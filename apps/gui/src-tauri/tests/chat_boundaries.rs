use p2p_chat::{ChatEnvelope, ChatFriend, ChatKind, ChatMediaMeta, ChatSendReport, ChatStatus, Sender, MAX_MESSAGE_SIZE};
use p2p_console::{chat::ChatMediaInputJson, util::to_asset_url};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

fn decode<T: DeserializeOwned>(v: Value) -> Result<T, serde_json::Error> { serde_json::from_value(v) }
fn envelope() -> ChatEnvelope { ChatEnvelope { id: "m".into(), peer: "peer".into(), sender: Sender::Me, kind: ChatKind::Image, ts_ms: 7, text: None, media: Some(ChatMediaMeta { name: "a b/中.png".into(), mime: "image/png".into(), size: 3, path: None }), status: ChatStatus::Pending, reply_to: None } }

#[test]
fn contract_roundtrips_all_chat_shapes_and_camel_case() {
    let friend = ChatFriend { peer_id: "p".into(), nickname: "n".into(), addrs: vec!["127.0.0.1/u1".into()], note: None };
    let report = ChatSendReport { message: envelope(), delivered: false };
    for value in [serde_json::to_value(&friend).expect("friend json"), serde_json::to_value(&report).expect("report json")] {
        assert!(value.get("peerId").is_some() || value.get("message").is_some(), "契约字段必须存在: {value}");
        assert!(value.get("peer_id").is_none(), "禁止 snake_case: {value}");
    }
    let value = serde_json::to_value(envelope()).expect("envelope json");
    assert_eq!(value["tsMs"], 7, "tsMs 必须 camelCase");
    assert_eq!(value["text"], Value::Null, "Option 必须序列化 null");
    assert_eq!(value["replyTo"], Value::Null, "无引用 replyTo 序列化 null（camelCase）");
    assert_eq!(value["media"]["path"], Value::Null, "媒体 path null 必须保留");
    let back: ChatEnvelope = decode(value).expect("envelope roundtrip");
    assert_eq!(back, envelope(), "全字段 roundtrip 丢失字段");
}

#[test]
fn json_missing_null_unknown_and_camel_case_are_explicit() {
    let full = json!({"id":"m","peer":"p","sender":"me","kind":"text","tsMs":1,"text":null,"media":null,"status":"pending","future":true});
    let parsed: ChatEnvelope = decode(full).expect("未知字段应兼容忽略");
    assert_eq!(parsed.text, None, "null text 应映射 None");
    assert_eq!(parsed.reply_to, None, "缺 replyTo 读回 = 无引用（旧格式容忍）");
    let with_reply: ChatEnvelope = decode(json!({"id":"m","peer":"p","sender":"me","kind":"text","tsMs":1,"text":"x","media":null,"status":"pending","replyTo":"q-1"})).expect("replyTo 输入兼容");
    assert_eq!(with_reply.reply_to.as_deref(), Some("q-1"), "replyTo 映射错误");
    assert!(decode::<ChatEnvelope>(json!({"id":"m"})).is_err(), "缺必填字段必须失败");
    assert!(decode::<ChatEnvelope>(json!({"id":"m","peer":"p","sender":"me","kind":"text","ts_ms":1,"text":"x","media":null,"status":"pending"})).is_err(), "snake_case 不得替代 camelCase");
    let media: ChatMediaInputJson = decode(json!({"name":"x","mime":"image/png","dataBase64":"eA==","extra":1})).expect("输入未知字段兼容");
    assert_eq!(media.data_base64, "eA==", "dataBase64 映射错误");
}

#[test]
fn peer_nickname_text_media_limits_have_readable_errors() {
    assert!(p2p_chat::validate_text(" ").expect_err("空文本必须拒绝").to_string().contains("文本"));
    assert!(p2p_chat::validate_text(&"x".repeat(2001)).expect_err("超长文本必须拒绝").to_string().contains("上限"));
    assert_eq!(p2p_chat::validate_text(" x ").expect("合法文本"), "x");
    assert!(p2p_chat::validate_media(&ChatKind::Image, "text/plain", 1).expect_err("MIME 错误").to_string().contains("MIME"));
    assert!(p2p_chat::validate_media(&ChatKind::File, "application/octet-stream", MAX_MESSAGE_SIZE + 1).expect_err("大小错误").to_string().contains("64MiB"));
    assert_eq!(p2p_chat::sanitize_name("../../a b.txt"), "....ab.txt", "文件名边界应可观测");
}

#[test]
fn asset_url_encodes_path_and_uses_platform_prefix() {
    let url = to_asset_url("/chat/media/a b/中.png");
    assert!(url.starts_with("asset://localhost/") || url.starts_with("http://asset.localhost/"), "平台前缀错误: {url}");
    assert!(url.contains("%2F") && url.contains("%20") && url.contains("%E4%B8%AD"), "路径必须按 URI component 编码: {url}");
    assert!(!url.trim_start_matches("asset://localhost/").contains('/'), "编码路径不得残留裸斜杠: {url}");
}
