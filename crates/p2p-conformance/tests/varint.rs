//! varint.json 消费：经 p2p-protocol 帧真实 API 交叉验证 LEB128 语义。

use p2p_conformance::{case_name, cases, load, ref_varint, unhex};
use p2p_protocol::{flatten_io, read_frame, write_frame, ProtocolError, MAX_FRAME_SIZE};
use std::io::Cursor;
use tokio::io::AsyncReadExt;

const FILE: &str = "varint.json";

/// 真实 write_frame 产帧，抓回全部字节（总长 = 参考前缀 + payload）。
async fn frame_bytes(len: u64) -> Vec<u8> {
    let payload = vec![0u8; len as usize];
    let (mut tx, mut rx) = tokio::io::duplex(64 * 1024);
    let writer = tokio::spawn(async move { write_frame(&mut tx, &payload).await });
    let total = ref_varint(len).len() + len as usize;
    let mut buf = vec![0u8; total];
    rx.read_exact(&mut buf).await.unwrap();
    writer.await.unwrap().unwrap();
    buf
}

#[tokio::test]
async fn golden_values_decode_via_read_frame() {
    let doc = load(FILE);
    for case in cases(&doc) {
        if case["valid"].as_bool() != Some(true) {
            continue;
        }
        let name = case_name(case);
        let bytes = unhex(case["bytes_hex"].as_str().unwrap());
        let value: u64 = case["value"].as_str().unwrap().parse().unwrap();
        if value <= MAX_FRAME_SIZE as u64 {
            // 小值走成功路径：前缀 + 恰好 value 字节 payload 完整读回
            let stream: Vec<u8> = bytes
                .iter()
                .copied()
                .chain(std::iter::repeat_n(0u8, value as usize))
                .collect();
            let got = read_frame(&mut Cursor::new(stream)).await.unwrap();
            assert_eq!(got.len() as u64, value, "case {name}: 读回长度不符");
            let frame = frame_bytes(value).await;
            assert_eq!(&frame[..bytes.len()], &bytes[..], "case {name}: 编码字节不符");
        } else {
            // 大值无法落地 payload：read_frame 应解码出正确长度后再因超帧上限拒绝，
            // FrameTooLarge 携带的值即 varint 解码结果，可精确核对
            let err = read_frame(&mut Cursor::new(bytes)).await.unwrap_err();
            match flatten_io(err) {
                ProtocolError::FrameTooLarge(len) => {
                    assert_eq!(len, value, "case {name}: 解码值回绕");
                }
                other => panic!("case {name}: 期望 FrameTooLarge，得到 {other:?}"),
            }
        }
    }
}

#[tokio::test]
async fn overflow_inputs_rejected_without_wraparound() {
    let doc = load(FILE);
    for case in cases(&doc) {
        if case["valid"].as_bool() != Some(false) {
            continue;
        }
        let name = case_name(case);
        let bytes = unhex(case["bytes_hex"].as_str().unwrap());
        let err = read_frame(&mut Cursor::new(bytes)).await.unwrap_err();
        assert_eq!(
            err.kind(),
            std::io::ErrorKind::InvalidData,
            "case {name}: 溢出输入必须 InvalidData"
        );
        assert!(err.to_string().contains("varint"), "case {name}: 错误缺可读信号");
    }
}
