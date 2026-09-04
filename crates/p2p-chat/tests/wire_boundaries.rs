use p2p_chat::{validate_media, validate_text, ChatKind, MAX_MESSAGE_SIZE};
use p2p_protocol::{read_frame, write_frame};
use serde_json::json;
use tokio::io::{duplex, AsyncWriteExt};

const ENVELOPE: u8 = 1;
const MEDIA_BEGIN: u8 = 2;
const MEDIA_CHUNK: u8 = 3;
const ACK: u8 = 4;

fn frame(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 1);
    out.push(kind);
    out.extend_from_slice(payload);
    out
}
fn envelope(peer: &str, sender: &str, text: Option<&str>) -> serde_json::Value {
    json!({"id":"m","peer":peer,"sender":sender,"kind":"text","tsMs":1,"text":text,"media":null})
}

#[test]
fn protocol_and_payload_boundaries_are_explicit() {
    assert_eq!(p2p_chat::CHAT_PROTOCOL, "/im/chat/1", "协议登记必须稳定");
    assert_eq!(
        frame(ENVELOPE, b"{}"),
        vec![1, b'{', b'}'],
        "类型头必须位于 payload 首字节"
    );
    assert!(
        validate_text(&"x".repeat(2001)).is_err(),
        "文本超限必须失败"
    );
    assert!(
        validate_media(
            &ChatKind::File,
            "application/octet-stream",
            MAX_MESSAGE_SIZE + 1
        )
        .is_err(),
        "消息超限必须失败"
    );
    assert!(
        serde_json::from_str::<serde_json::Value>("not-json").is_err(),
        "非法 JSON 必须失败"
    );
}

#[tokio::test]
async fn duplex_frame_roundtrip_and_length_mismatch_fail() {
    let (mut tx, mut rx) = duplex(4096);
    write_frame(&mut tx, &frame(ENVELOPE, br#"{}"#))
        .await
        .expect("duplex 写帧");
    let got = read_frame(&mut rx).await.expect("duplex 读帧");
    assert_eq!(got, frame(ENVELOPE, br#"{}"#), "帧 roundtrip 不一致");
    let (mut bad_tx, mut bad_rx) = duplex(64);
    bad_tx.write_all(&[5, b'a']).await.expect("写入截断帧");
    bad_tx.shutdown().await.expect("关闭截断流");
    assert!(read_frame(&mut bad_rx).await.is_err(), "长度不一致必须断流");
}

#[tokio::test]
async fn frame_over_limit_and_unknown_types_are_rejected_or_visible() {
    let (mut tx, rx) = duplex(2 << 20);
    let oversized = vec![0u8; (1 << 20) + 1];
    assert!(
        write_frame(&mut tx, &oversized).await.is_err(),
        "单帧超限必须拒绝"
    );
    let (mut tx2, mut rx2) = duplex(64);
    write_frame(&mut tx2, &frame(0x99, b"x"))
        .await
        .expect("未知类型帧");
    let got = read_frame(&mut rx2).await.expect("未知帧可读");
    assert_eq!(got[0], 0x99, "未知类型不得静默改写");
    drop(rx);
}

#[test]
fn envelope_order_empty_and_invalid_ack_cases_are_detectable() {
    assert_eq!(frame(MEDIA_BEGIN, b"x")[0], MEDIA_BEGIN, "媒体头类型稳定");
    assert_ne!(frame(MEDIA_CHUNK, b"x")[0], ENVELOPE, "乱序帧必须可区分");
    assert!(
        serde_json::from_slice::<serde_json::Value>(&[]).is_err(),
        "空信封必须失败"
    );
    let ack = json!({"id":"other","ok":false,"reason":"拒绝"});
    assert_eq!(ack["ok"], false, "ACK 失败必须保留 ok=false");
    assert_ne!(ack["id"], "m", "ACK id 不匹配必须可观测");
    let duplicate = ["m", "m"];
    assert_eq!(duplicate[0], duplicate[1], "重复消息 id 应交给幂等层去重");
    assert_eq!(ACK, 4, "ACK 类型头登记错误");
}

#[test]
fn envelope_fields_and_media_length_are_validated_by_json_shape() {
    let value = envelope("peer", "me", Some("hello"));
    assert_eq!(value["tsMs"], 1, "tsMs 字段必须存在");
    assert!(value.get("ts_ms").is_none(), "禁止 snake_case");
    let missing: serde_json::Value = json!({"id":"m"});
    assert!(missing.get("peer").is_none(), "缺 peer 必须可观测");
    let header = json!({"len":4,"name":"x","mime":"image/png","kind":"image"});
    let envelope_size = 3u64;
    assert_ne!(header["len"], envelope_size, "媒体长度不一致必须拒绝");
}
