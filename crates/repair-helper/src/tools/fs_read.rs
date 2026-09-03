//! fs_read：授权根内单文件文本读取（read 档）。
//!
//! 路径经 [crate::jail::PathJail] 解析，越界即拒带原因；按输出门禁上限边界
//! 读取（take 上限+1），超大/二进制文件不炸内存；NUL 字节判二进制并加标记。

use std::fs;
use std::io::Read;

use async_trait::async_trait;
use serde_json::Value;

use crate::cap::{self, MAX_OUTPUT_BYTES};
use crate::jail::PathJail;
use crate::{Tool, ToolResult};

pub struct FsRead {
    jail: PathJail,
}

impl FsRead {
    pub fn new(jail: PathJail) -> Self {
        Self { jail }
    }
}

#[async_trait]
impl Tool for FsRead {
    fn name(&self) -> &str {
        "fs_read"
    }

    fn description(&self) -> &str {
        "读取授权根内单文件文本内容（二进制安全、超限截断，只读）"
    }

    async fn call(&self, arguments: Value) -> Result<ToolResult, String> {
        let raw = arguments
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| "fs_read: missing string param 'path'".to_string())?;
        let resolved = self
            .jail
            .resolve(raw)
            .map_err(|e| format!("fs_read: {e}"))?;
        let meta = fs::metadata(&resolved).map_err(|e| format!("fs_read: metadata: {e}"))?;
        if meta.is_dir() {
            return Err("fs_read: path is a directory".into());
        }
        let mut file = fs::File::open(&resolved).map_err(|e| format!("fs_read: open: {e}"))?;
        let mut buf = Vec::new();
        file.by_ref()
            .take(MAX_OUTPUT_BYTES as u64 + 1)
            .read_to_end(&mut buf)
            .map_err(|e| format!("fs_read: read: {e}"))?;
        let over = buf.len() > MAX_OUTPUT_BYTES;
        if over {
            buf.truncate(MAX_OUTPUT_BYTES);
        }
        let binary = buf.contains(&0);
        let mut text = String::from_utf8_lossy(&buf).into_owned();
        if binary {
            text = format!("binary=1 bytes={}\n{text}", meta.len());
        }
        Ok(cap::apply_output_gate(ToolResult {
            text,
            truncated: over,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(tag: &str) -> (std::path::PathBuf, PathJail) {
        let root = std::env::temp_dir().join(format!("rh-fsr-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        (root.clone(), PathJail::from_roots(vec![root]).unwrap())
    }

    #[tokio::test]
    async fn happy_path() {
        let (root, jail) = fixture("ok");
        std::fs::write(root.join("a.txt"), "hello").unwrap();
        let result = FsRead::new(jail)
            .call(json!({"path": "a.txt"}))
            .await
            .unwrap();
        assert_eq!(result.text, "hello");
        assert!(!result.truncated);
    }

    #[tokio::test]
    async fn escape_rejected_with_reason() {
        let (root, jail) = fixture("esc");
        let outside = root.parent().unwrap().join("outside.txt");
        std::fs::write(&outside, "x").unwrap();
        let err = FsRead::new(jail)
            .call(json!({"path": "../outside.txt"}))
            .await
            .unwrap_err();
        assert!(err.contains(".."), "unexpected: {err}");
        let _ = std::fs::remove_file(outside);
    }

    #[tokio::test]
    async fn binary_safe_and_marked() {
        let (root, jail) = fixture("bin");
        std::fs::write(root.join("b.bin"), [0x00u8, 0x01, b'a', 0xff]).unwrap();
        let result = FsRead::new(jail)
            .call(json!({"path": "b.bin"}))
            .await
            .unwrap();
        assert!(result.text.contains("binary=1"), "{}", result.text);
    }

    #[tokio::test]
    async fn huge_file_truncated_not_oom() {
        let (root, jail) = fixture("huge");
        std::fs::write(root.join("big.txt"), "a".repeat(MAX_OUTPUT_BYTES + 4096)).unwrap();
        let result = FsRead::new(jail)
            .call(json!({"path": "big.txt"}))
            .await
            .unwrap();
        assert!(result.truncated);
        assert!(result.text.len() <= MAX_OUTPUT_BYTES);
    }

    #[tokio::test]
    async fn missing_or_wrong_param_rejected() {
        let (_, jail) = fixture("bad");
        let tool = FsRead::new(jail);
        assert!(tool.call(json!({})).await.is_err());
        assert!(tool.call(json!({"path": 42})).await.is_err());
    }
}
