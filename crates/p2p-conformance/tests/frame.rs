//! frame.json 消费：帧封装、1 MiB 上限与长度不符断流语义。

mod common;
use common::{case_name, cases, load, ref_frame, ref_varint, unhex};
use p2p_protocol::read_frame;
use std::io::Cursor;

const FILE: &str = "frame.json";

#[tokio::test]
async fn golden_frames_roundtrip_via_read_frame() {
    let doc = load(FILE);
    for case in cases(&doc) {
        let (Some(payload_hex), Some(frame_hex)) =
            (case["payload_hex"].as_str(), case["frame_hex"].as_str())
        else {
            continue;
        };
        let name = case_name(case);
        let payload = unhex(payload_hex);
        let got = read_frame(&mut Cursor::new(unhex(frame_hex)))
            .await
            .unwrap();
        assert_eq!(got, payload, "case {name}: 帧解码不符");
        // 参考封装与向量 frame_hex 一致（编码语义双向锁定）
        assert_eq!(
            ref_frame(&payload),
            unhex(frame_hex),
            "case {name}: 编码不符"
        );
    }
}

#[tokio::test]
async fn max_size_boundary_is_accepted() {
    let doc = load(FILE);
    let case = cases(&doc)
        .into_iter()
        .find(|c| case_name(c) == "frame_max_size_boundary")
        .expect("frame_max_size_boundary case");
    let prefix = unhex(case["frame_prefix_hex"].as_str().unwrap());
    let declared: usize = case["declared_len"].as_u64().unwrap() as usize;
    let repeat = unhex(case["payload_repeat_hex"].as_str().unwrap());
    assert_eq!(prefix, ref_varint(declared as u64), "上限前缀编码不符");
    let mut stream = prefix;
    stream.extend(std::iter::repeat_n(repeat[0], declared));
    let got = read_frame(&mut Cursor::new(stream)).await.unwrap();
    assert_eq!(got.len(), declared, "恰在上限的帧必须完整读回");
}

#[tokio::test]
async fn over_limit_length_rejected_before_payload_read() {
    let doc = load(FILE);
    let case = cases(&doc)
        .into_iter()
        .find(|c| case_name(c) == "frame_over_limit_rejected")
        .expect("frame_over_limit_rejected case");
    // 线上只有几个垃圾字节即可触发：长度检查在读取 payload 之前
    let mut stream = unhex(case["frame_prefix_hex"].as_str().unwrap());
    stream.extend_from_slice(b"junk");
    let err = read_frame(&mut Cursor::new(stream)).await.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("too large"),
        "错误缺可读信号: {err}"
    );
}

#[tokio::test]
async fn truncated_stream_errors_instead_of_short_frame() {
    let doc = load(FILE);
    let case = cases(&doc)
        .into_iter()
        .find(|c| case_name(c) == "frame_length_payload_mismatch_truncates")
        .expect("frame_length_payload_mismatch_truncates case");
    let declared: usize = case["declared_len"].as_u64().unwrap() as usize;
    let available = unhex(case["available_payload_hex"].as_str().unwrap());
    assert!(available.len() < declared, "样例必须是截断形态");
    let mut stream = ref_varint(declared as u64);
    stream.extend_from_slice(&available);
    let err = read_frame(&mut Cursor::new(stream)).await.unwrap_err();
    assert_eq!(
        err.kind(),
        std::io::ErrorKind::UnexpectedEof,
        "长度与实际不符必须断流（EOF），不得静默交付短帧"
    );
}
