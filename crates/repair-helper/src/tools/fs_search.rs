//! fs_search：授权根内按文件名通配（* ?）递归搜索（read 档）。
//!
//! 输出命中文件名相对路径（按名称排序）；不跟随符号链接目录防循环；
//! 目录访问有上限防病态树；结果受输出门禁约束。

use std::fs;

use async_trait::async_trait;
use serde_json::Value;
use tracing::warn;

use crate::cap;
use crate::jail::PathJail;
use crate::tools::glob;
use crate::{Tool, ToolResult};

/// 目录访问上限，防病态目录树下无限遍历。
const MAX_VISITED_DIRS: usize = 20_000;

pub struct FsSearch {
    jail: PathJail,
}

impl FsSearch {
    pub fn new(jail: PathJail) -> Self {
        Self { jail }
    }
}

#[async_trait]
impl Tool for FsSearch {
    fn name(&self) -> &str {
        "fs_search"
    }

    fn description(&self) -> &str {
        "在授权根内按文件名通配（* ?）递归搜索，输出相对路径（只读）"
    }

    async fn call(&self, arguments: Value) -> Result<ToolResult, String> {
        let pattern = arguments
            .get("pattern")
            .and_then(Value::as_str)
            .ok_or_else(|| "fs_search: missing string param 'pattern'".to_string())?;
        let root = match arguments.get("root").and_then(Value::as_str) {
            Some(raw) => self
                .jail
                .resolve(raw)
                .map_err(|e| format!("fs_search: {e}"))?,
            None => self
                .jail
                .first_root()
                .map_err(|e| format!("fs_search: {e}"))?,
        };
        let meta = fs::metadata(&root).map_err(|e| format!("fs_search: metadata: {e}"))?;
        if !meta.is_dir() {
            return Err("fs_search: root is not a directory".into());
        }
        let mut hits = Vec::new();
        let mut stack = vec![root.clone()];
        let mut visited = 0usize;
        while let Some(dir) = stack.pop() {
            visited += 1;
            if visited > MAX_VISITED_DIRS {
                break;
            }
            let rd = match fs::read_dir(&dir) {
                Ok(rd) => rd,
                Err(e) => {
                    warn!(error = %e, "fs_search: read_dir skipped");
                    continue;
                }
            };
            for item in rd {
                let item = match item {
                    Ok(i) => i,
                    Err(e) => {
                        warn!(error = %e, "fs_search: entry skipped");
                        continue;
                    }
                };
                let ft = match item.file_type() {
                    Ok(t) => t,
                    Err(e) => {
                        warn!(error = %e, "fs_search: file_type skipped");
                        continue;
                    }
                };
                if ft.is_dir() && !ft.is_symlink() {
                    stack.push(item.path());
                }
                let name = item.file_name().to_string_lossy().into_owned();
                if ft.is_file() && glob::matches(&name, pattern) {
                    if let Ok(rel) = item.path().strip_prefix(&root) {
                        hits.push(rel.to_string_lossy().into_owned());
                    }
                }
            }
        }
        hits.sort();
        let mut text = String::new();
        for hit in &hits {
            text.push_str(hit);
            text.push('\n');
        }
        if visited > MAX_VISITED_DIRS {
            text.push_str("# visit limit reached\n");
        }
        Ok(cap::apply_output_gate(ToolResult {
            text,
            truncated: false,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(tag: &str) -> (std::path::PathBuf, PathJail) {
        let root = std::env::temp_dir().join(format!("rh-fss-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("alpha.txt"), "hello").unwrap();
        std::fs::write(root.join("sub").join("beta.log"), "world").unwrap();
        std::fs::write(root.join("sub").join("gamma.tmp"), "tmp").unwrap();
        (root.clone(), PathJail::from_roots(vec![root]).unwrap())
    }

    #[tokio::test]
    async fn hits_and_empty() {
        let (_, jail) = fixture("ok");
        let tool = FsSearch::new(jail);
        let hits = tool.call(json!({"pattern": "*.log"})).await.unwrap();
        assert_eq!(hits.text.trim(), "sub/beta.log");
        assert!(!hits.truncated);
        let empty = tool.call(json!({"pattern": "*.zzz"})).await.unwrap();
        assert!(empty.text.trim().is_empty());
    }

    #[tokio::test]
    async fn nested_relative_paths() {
        let (_, jail) = fixture("nested");
        let hits = FsSearch::new(jail)
            .call(json!({"pattern": "*.txt"}))
            .await
            .unwrap();
        assert!(hits.text.contains("alpha.txt"), "{}", hits.text);
    }

    #[tokio::test]
    async fn explicit_root_subdir() {
        let (_, jail) = fixture("root");
        let hits = FsSearch::new(jail)
            .call(json!({"pattern": "*.log", "root": "sub"}))
            .await
            .unwrap();
        assert_eq!(hits.text.trim(), "beta.log");
    }

    #[tokio::test]
    async fn missing_pattern_rejected() {
        let (_, jail) = fixture("nop");
        assert!(FsSearch::new(jail).call(json!({})).await.is_err());
    }
}
