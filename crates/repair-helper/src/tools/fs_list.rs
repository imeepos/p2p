//! fs_list：授权根内目录列举（read 档）。
//!
//! 输出一行一条：名称/类型/大小/mtime，tab 分隔、按名称排序；
//! 目录大小记 "-"，mtime 为 Unix 秒；不可得字段记 "-" 并留 warn 日志。

use std::fs;
use std::time::UNIX_EPOCH;

use async_trait::async_trait;
use serde_json::Value;
use tracing::warn;

use crate::cap;
use crate::jail::PathJail;
use crate::{Tool, ToolResult};

pub struct FsList {
    jail: PathJail,
}

impl FsList {
    pub fn new(jail: PathJail) -> Self {
        Self { jail }
    }
}

#[async_trait]
impl Tool for FsList {
    fn name(&self) -> &str {
        "fs_list"
    }

    fn description(&self) -> &str {
        "列举授权根内目录条目：名称/类型/大小/mtime，按名称排序（只读）"
    }

    async fn call(&self, arguments: Value) -> Result<ToolResult, String> {
        let raw = arguments
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| "fs_list: missing string param 'path'".to_string())?;
        let resolved = self
            .jail
            .resolve(raw)
            .map_err(|e| format!("fs_list: {e}"))?;
        let meta = fs::metadata(&resolved).map_err(|e| format!("fs_list: metadata: {e}"))?;
        if !meta.is_dir() {
            return Err("fs_list: path is not a directory".into());
        }
        let rd = fs::read_dir(&resolved).map_err(|e| format!("fs_list: read_dir: {e}"))?;
        let mut entries = Vec::new();
        for item in rd {
            let item = match item {
                Ok(i) => i,
                Err(e) => {
                    warn!(error = %e, "fs_list: entry read skipped");
                    continue;
                }
            };
            let ft = match item.file_type() {
                Ok(t) => t,
                Err(e) => {
                    warn!(error = %e, "fs_list: file_type skipped");
                    continue;
                }
            };
            let (size, mtime) = match item.metadata() {
                Ok(m) => (
                    if ft.is_file() {
                        m.len().to_string()
                    } else {
                        "-".into()
                    },
                    mtime_str(&m),
                ),
                Err(e) => {
                    warn!(error = %e, "fs_list: metadata skipped");
                    ("-".into(), "-".into())
                }
            };
            entries.push(EntryInfo {
                name: item.file_name().to_string_lossy().into_owned(),
                kind: kind_name(&ft).to_string(),
                size,
                mtime,
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let mut text = String::new();
        for e in &entries {
            text.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                e.name, e.kind, e.size, e.mtime
            ));
        }
        Ok(cap::apply_output_gate(ToolResult {
            text,
            truncated: false,
        }))
    }
}

struct EntryInfo {
    name: String,
    kind: String,
    size: String,
    mtime: String,
}

fn kind_name(ft: &fs::FileType) -> &'static str {
    if ft.is_dir() {
        "dir"
    } else if ft.is_file() {
        "file"
    } else if ft.is_symlink() {
        "symlink"
    } else {
        "other"
    }
}

fn mtime_str(meta: &fs::Metadata) -> String {
    match meta.modified() {
        Ok(t) => t
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "-".into()),
        Err(e) => {
            warn!(error = %e, "fs_list: modified unavailable");
            "-".into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(tag: &str) -> (std::path::PathBuf, PathJail) {
        let root = std::env::temp_dir().join(format!("rh-fsl-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("alpha.txt"), "hello").unwrap();
        (root.clone(), PathJail::from_roots(vec![root]).unwrap())
    }

    #[tokio::test]
    async fn entries_with_kind_size() {
        let (_, jail) = fixture("ok");
        let result = FsList::new(jail).call(json!({"path": "."})).await.unwrap();
        assert!(
            result.text.contains("alpha.txt\tfile\t5\t"),
            "{}",
            result.text
        );
        assert!(result.text.contains("sub\tdir\t-\t"), "{}", result.text);
    }

    #[tokio::test]
    async fn non_dir_rejected() {
        let (_, jail) = fixture("file");
        let err = FsList::new(jail)
            .call(json!({"path": "alpha.txt"}))
            .await
            .unwrap_err();
        assert!(err.contains("not a directory"), "unexpected: {err}");
    }

    #[test]
    fn kind_and_mtime_helpers() {
        let (root, _) = fixture("helpers");
        let meta = std::fs::metadata(root.join("alpha.txt")).unwrap();
        let mtime = mtime_str(&meta);
        assert!(mtime.parse::<u64>().is_ok(), "mtime {mtime}");
        let ft = std::fs::metadata(root.join("sub")).unwrap().file_type();
        assert_eq!(kind_name(&ft), "dir");
    }
}
