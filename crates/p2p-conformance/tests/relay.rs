//! relay-messages.json 消费：RelayMsg oneof tag 1-9 黄金字节与错误码 1-7 样例。

mod common;
use common::{case_name, cases, load, ref_frame, unhex};
use p2p_relay::frame::read_msg;
use p2p_relay::messages::{relay_msg, RelayMsg};
use prost::Message;
use std::io::Cursor;

const FILE: &str = "relay-messages.json";

fn kind_tag(kind: &relay_msg::Kind) -> u64 {
    match kind {
        relay_msg::Kind::Reserve(_) => 1,
        relay_msg::Kind::Reserved(_) => 2,
        relay_msg::Kind::Connect(_) => 3,
        relay_msg::Kind::Bound(_) => 4,
        relay_msg::Kind::PunchReq(_) => 5,
        relay_msg::Kind::PunchAck(_) => 6,
        relay_msg::Kind::Reject(_) => 7,
        relay_msg::Kind::KeepAlive(_) => 8,
        relay_msg::Kind::KeepAliveAck(_) => 9,
    }
}

fn expected_tag(name: &str) -> u64 {
    match name {
        "reserve" => 1,
        "reserved" => 2,
        "connect" => 3,
        "bound" => 4,
        "punch_req" => 5,
        "punch_ack" => 6,
        "keep_alive" => 8,
        "keep_alive_ack" => 9,
        n if n.starts_with("reject_code_") => {
            let code: u64 = n
                .trim_start_matches("reject_code_")
                .split('_')
                .next()
                .unwrap()
                .parse()
                .unwrap();
            assert!((1..=7).contains(&code), "错误码样例须在 1-7: {n}");
            7
        }
        other => panic!("未知向量 case: {other}"),
    }
}

#[test]
fn golden_bytes_roundtrip_all_tags() {
    let doc = load(FILE);
    for case in cases(&doc) {
        let name = case_name(case);
        let protobuf = unhex(case["protobuf_hex"].as_str().unwrap());
        let frame = unhex(case["frame_hex"].as_str().unwrap());
        assert_eq!(ref_frame(&protobuf), frame, "case {name}: 长度前缀不符");

        let msg = RelayMsg::decode(protobuf.as_slice()).expect("case {name}: protobuf 解码失败");
        let kind = msg
            .kind
            .as_ref()
            .unwrap_or_else(|| panic!("case {name}: 缺 kind"));
        assert_eq!(
            kind_tag(kind),
            expected_tag(name),
            "case {name}: oneof tag 不符"
        );
        assert_eq!(
            msg.encode_to_vec(),
            protobuf,
            "case {name}: 重编码不是规范形（prost3 零值省略语义漂移）"
        );
        if let relay_msg::Kind::Reject(reject) = kind {
            let code: u64 = name
                .trim_start_matches("reject_code_")
                .split('_')
                .next()
                .unwrap()
                .parse()
                .unwrap();
            assert_eq!(reject.code as u64, code, "case {name}: 错误码不符");
        }
    }
}

#[tokio::test]
async fn framed_vectors_read_back_via_read_msg() {
    let doc = load(FILE);
    for case in cases(&doc) {
        let name = case_name(case);
        let frame = unhex(case["frame_hex"].as_str().unwrap());
        let expect =
            RelayMsg::decode(unhex(case["protobuf_hex"].as_str().unwrap()).as_slice()).unwrap();
        let got = read_msg(&mut Cursor::new(frame))
            .await
            .unwrap()
            .unwrap_or_else(|| panic!("case {name}: 帧读空"));
        assert_eq!(got, expect, "case {name}: 帧读取不符");
    }
}

/// 构造器路径与向量交叉：9 个 tag 的便捷构造必须产出黄金字节（reject 以解码值重建）。
#[test]
fn constructors_reproduce_golden_bytes() {
    let doc = load(FILE);
    for case in cases(&doc) {
        let name = case_name(case);
        let protobuf = unhex(case["protobuf_hex"].as_str().unwrap());
        let rebuilt = match name {
            "reserve" => RelayMsg::reserve(300, "peer-b"),
            "reserved" => RelayMsg::reserved(7, 500),
            "connect" => RelayMsg::connect(7),
            "bound" => RelayMsg::bound(7),
            "punch_req" => RelayMsg::punch_req("peer-b", vec!["203.0.113.7:4001".into()]),
            "punch_ack" => RelayMsg::punch_ack(
                "peer-a",
                vec!["198.51.100.9:7777".into(), "[2001:db8::1]:4444".into()],
            ),
            "keep_alive" => RelayMsg::keep_alive(),
            "keep_alive_ack" => RelayMsg::keep_alive_ack(500),
            n if n.starts_with("reject_code_") => {
                let reject = match RelayMsg::decode(protobuf.as_slice()).unwrap().kind.unwrap() {
                    relay_msg::Kind::Reject(r) => r,
                    other => panic!("case {n}: 期望 Reject，得到 {other:?}"),
                };
                RelayMsg::error(reject.code, reject.message)
            }
            other => panic!("未知向量 case: {other}"),
        };
        assert_eq!(
            rebuilt.encode_to_vec(),
            protobuf,
            "case {name}: 构造器编码不符"
        );
    }
}
