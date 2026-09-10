//! im-chat-envelope.json 消费：ENVELOPE/ACK 帧字节、信封 JSON 与 MIME 白名单。

use p2p_chat::{validate_media, ChatKind};
use p2p_conformance::{case_name, cases, load, ref_frame, unhex};
use p2p_protocol::read_frame;
use serde_json::Value;
use std::io::Cursor;

const FILE: &str = "im-chat-envelope.json";

async fn read_typed(frame: Vec<u8>) -> (u8, Vec<u8>) {
    let got = read_frame(&mut Cursor::new(frame)).await.expect("帧读失败");
    (got[0], got[1..].to_vec())
}

#[tokio::test]
async fn envelope_and_ack_frames_match_vectors() {
    let doc = load(FILE);
    for case in cases(&doc) {
        let Some(frame_hex) = case["frame_hex"].as_str() else {
            continue;
        };
        let name = case_name(case);
        let head = unhex(case["type_byte"].as_str().unwrap())[0];
        // payload_hex 语义 = 类型头 + JSON 载荷字节
        let payload = unhex(case["payload_hex"].as_str().unwrap());
        assert_eq!(payload[0], head, "case {name}: payload 首字节须为类型头");
        let payload_json: Value =
            serde_json::from_str(case["payload_json"].as_str().unwrap()).unwrap();

        // 编码方向：类型头 + payload 规范封装 == 向量整帧字节
        assert_eq!(ref_frame(&payload), unhex(frame_hex), "case {name}: 整帧字节不符");

        // 解码方向：向量帧读回的类型头与 JSON 载荷语义一致
        let (got_head, got_payload) = read_typed(unhex(frame_hex)).await;
        assert_eq!(got_head, head, "case {name}: 类型头不符");
        let got_json: Value = serde_json::from_slice(&got_payload).expect("case {name}: JSON 非法");
        assert_eq!(got_json, payload_json, "case {name}: 载荷 JSON 语义不符");
    }
}

#[tokio::test]
async fn envelope_semantics_and_impostor_condition() {
    let doc = load(FILE);
    for case in cases(&doc) {
        let Some(json_str) = case["payload_json"].as_str() else {
            continue;
        };
        let name = case_name(case);
        let json: Value = serde_json::from_str(json_str).unwrap();
        if !case["valid"].as_bool().unwrap_or(true) {
            // 负样例：sender 非 me 即触发收端断流拒绝（该校验在收端 handler 内部）
            assert_ne!(
                json["sender"].as_str().unwrap_or_default(),
                "me",
                "case {name}: 伪装样例须以 sender != me 表达"
            );
            continue;
        }
        if case["frame_type"].as_str() == Some("ENVELOPE") {
            assert_eq!(json["sender"], "me", "case {name}: 合法信封 sender 必须为 me");
            assert!(json["id"].as_str().is_some(), "case {name}: 缺 id");
            assert!(json["peer"].as_str().is_some(), "case {name}: 缺 peer");
            assert!(json["tsMs"].is_i64(), "case {name}: tsMs 须为 i64");
        }
        if case["frame_type"].as_str() == Some("ACK") {
            assert_eq!(json["ok"], true, "case {name}: ACK 样例须 ok=true");
            assert!(json["id"].as_str().is_some(), "case {name}: 缺 id");
        }
    }
}

#[test]
fn mime_whitelist_matrix_matches_vectors() {
    let doc = load(FILE);
    for case in cases(&doc) {
        let (Some(kind), Some(mime), Some(size)) = (
            case["kind"].as_str(),
            case["mime"].as_str(),
            case["size"].as_str(),
        ) else {
            continue;
        };
        let name = case_name(case);
        let chat_kind = match kind {
            "text" => ChatKind::Text,
            "image" => ChatKind::Image,
            "audio" => ChatKind::Audio,
            "video" => ChatKind::Video,
            "file" => ChatKind::File,
            "groupinvite" => ChatKind::GroupInvite,
            other => panic!("case {name}: 未知 kind {other}"),
        };
        let verdict = validate_media(&chat_kind, mime, size.parse().unwrap());
        assert_eq!(
            verdict.is_ok(),
            case["valid"].as_bool().unwrap(),
            "case {name}: MIME 白名单裁决与向量不符"
        );
    }
}
