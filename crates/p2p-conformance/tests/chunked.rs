//! chunked.json 消费：类型序、重组与上限语义（小规模等价用例）。

mod common;
use common::{case_name, cases, load, ref_frame, unhex};
use p2p_protocol::{flatten_io, read_chunked, ProtocolError};
use std::io::Cursor;

const FILE: &str = "chunked.json";

fn stream_of(frame_payloads: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for frame in frame_payloads {
        out.extend_from_slice(&ref_frame(frame));
    }
    out
}

#[tokio::test]
async fn golden_sequences_reassemble() {
    let doc = load(FILE);
    for case in cases(&doc) {
        let Some(message_hex) = case["message_hex"].as_str() else {
            continue;
        };
        let name = case_name(case);
        let frames: Vec<Vec<u8>> = case["frames"]
            .as_array()
            .expect("frames 数组")
            .iter()
            .map(|f| unhex(f.as_str().unwrap()))
            .collect();
        let refs: Vec<&[u8]> = frames.iter().map(|v| v.as_slice()).collect();
        let msg = read_chunked(&mut Cursor::new(stream_of(&refs)))
            .await
            .unwrap();
        assert_eq!(hex_of(&msg), message_hex, "case {name}: 重组结果不符");
    }
}

#[tokio::test]
async fn illegal_sequences_rejected() {
    let doc = load(FILE);
    for case in cases(&doc) {
        if case["valid"].as_bool() != Some(false) || case["frames"].is_null() {
            continue;
        }
        let name = case_name(case);
        let frames: Vec<Vec<u8>> = case["frames"]
            .as_array()
            .expect("frames 数组")
            .iter()
            .map(|f| unhex(f.as_str().unwrap()))
            .collect();
        let refs: Vec<&[u8]> = frames.iter().map(|v| v.as_slice()).collect();
        let err = read_chunked(&mut Cursor::new(stream_of(&refs)))
            .await
            .unwrap_err();
        expect_rejected(name, &err);
    }
}

fn expect_rejected(name: &str, err: &std::io::Error) {
    if name == "truncated_before_end" {
        // 截断：重组未完成即 EOF
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof, "case {name}");
    } else {
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData, "case {name}");
    }
}

/// 重组上限等价用例：64 个满载 CHUNK + 带 65 字节的 END，累计超 64 MiB 即红。
#[tokio::test]
async fn reassembly_over_limit_rejected() {
    let doc = load(FILE);
    let case = cases(&doc)
        .into_iter()
        .find(|c| case_name(c) == "reassembly_over_limit_rejected")
        .expect("reassembly_over_limit_rejected case");
    let chunk_len: usize = case["chunk_data_len"].as_u64().unwrap() as usize;
    let chunk_frames: usize = case["chunk_frames"].as_u64().unwrap() as usize;
    let end_len: usize = case["end_data_len"].as_u64().unwrap() as usize;
    let chunk_data = unhex(case["chunk_data_repeat_hex"].as_str().unwrap());
    let end_data = unhex(case["end_data_repeat_hex"].as_str().unwrap());

    let mut stream = Vec::new();
    for _ in 0..chunk_frames {
        let mut frame = vec![0x01];
        frame.extend(std::iter::repeat_n(chunk_data[0], chunk_len));
        stream.extend_from_slice(&ref_frame(&frame));
    }
    let mut end = vec![0x02];
    end.extend(std::iter::repeat_n(end_data[0], end_len));
    stream.extend_from_slice(&ref_frame(&end));

    let err = read_chunked(&mut Cursor::new(stream)).await.unwrap_err();
    match flatten_io(err) {
        ProtocolError::MessageTooLarge(total) => {
            assert_eq!(total, (chunk_frames * chunk_len + end_len) as u64);
        }
        other => panic!("期望 MessageTooLarge，得到 {other:?}"),
    }
}

fn hex_of(bytes: &[u8]) -> String {
    common::hex(bytes)
}
