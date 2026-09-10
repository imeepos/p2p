//! 符合性向量消费公共件（纯测试型 crate，无业务逻辑）。
//!
//! 向量唯一源是 docs/protocol/vectors/*.json；本 crate 经 CARGO_MANIFEST_DIR
//! 相对定位读取，不把任何向量数据复制进源码。
//! 仅有的两个参考函数（ref_varint/ref_frame）是帧封装语义的内存参考实现，
//! 其正确性由 varint.json/frame.json 用例与 p2p-protocol 真实 API 交叉锁定。

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

/// 章程 §8 冻结的向量集文件名清单。
pub const VECTOR_FILES: [&str; 8] = [
    "varint.json",
    "frame.json",
    "peer-id.json",
    "chunked.json",
    "rendezvous-register.json",
    "relay-messages.json",
    "im-chat-envelope.json",
    "handshake-identity.json",
];

pub fn vectors_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/protocol/vectors")
}

pub fn load(file: &str) -> Value {
    let path = vectors_dir().join(file);
    let raw =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("向量文件不可读 {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("向量 JSON 非法 {}: {e}", path.display()))
}

pub fn cases(doc: &Value) -> Vec<&Value> {
    doc["cases"]
        .as_array()
        .unwrap_or_else(|| panic!("{} 缺 cases 数组", doc["vector_set"]))
        .iter()
        .collect()
}

pub fn case_name(case: &Value) -> &str {
    case["name"].as_str().expect("case.name")
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn unhex(s: &str) -> Vec<u8> {
    let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(clean.len().is_multiple_of(2), "hex 长度须为偶数: {s}");
    (0..clean.len() / 2)
        .map(|i| u8::from_str_radix(&clean[i * 2..i * 2 + 2], 16).expect("非法 hex 字符"))
        .collect()
}

/// 最短 LEB128 无符号编码（参考实现；语义由 varint.json 锁定）。
pub fn ref_varint(mut v: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return out;
        }
        out.push(b | 0x80);
    }
}

/// 帧封装参考实现：varint(len)+payload（语义由 frame.json 锁定）。
pub fn ref_frame(payload: &[u8]) -> Vec<u8> {
    let mut out = ref_varint(payload.len() as u64);
    out.extend_from_slice(payload);
    out
}
