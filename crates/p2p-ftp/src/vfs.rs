//! 文件系统抽象与列表编解码：服务端可插拔后端（本地盘 jail / 未来存储后端）。
//!
//! 路径一律为服务端虚拟路径（以 `/` 起始，根 = 后端根目录）；越狱防护
//! 由具体后端负责（[crate::LocalFs] 以组件过滤 + canonicalize 双闸实现）。

use std::io;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub kind: EntryKind,
    pub size: u64,
    pub mtime_unix: u64,
}

#[async_trait]
pub trait FileSystem: Send + Sync {
    async fn list(&self, vpath: &str) -> io::Result<Vec<Entry>>;
    async fn metadata(&self, vpath: &str) -> io::Result<Entry>;
    async fn mkdir(&self, vpath: &str) -> io::Result<()>;
    async fn remove_dir(&self, vpath: &str) -> io::Result<()>;
    async fn remove_file(&self, vpath: &str) -> io::Result<()>;
    async fn rename(&self, from: &str, to: &str) -> io::Result<()>;
    async fn reader(&self, vpath: &str) -> io::Result<Box<dyn AsyncRead + Unpin + Send>>;
    /// `append=false` 时目标已存在则截断（STOR 语义）。
    async fn writer(
        &self,
        vpath: &str,
        append: bool,
    ) -> io::Result<Box<dyn AsyncWrite + Unpin + Send>>;
}

/// 虚拟路径规范化：`base` + `arg` 折叠掉 `.` 与空段；越出根（`..` 上溢）
/// 返回 None（会话层回 550，不判形态直接拒）。
pub fn normalize(base: &str, arg: &str) -> Option<String> {
    let combined = if arg.starts_with('/') {
        arg.to_string()
    } else {
        format!("{base}/{arg}")
    };
    let mut parts: Vec<&str> = Vec::new();
    for seg in combined.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return None;
                }
            }

            s => parts.push(s),
        }
    }
    Some(format!("/{}", parts.join("/")))
}

/// LIST 明细行编码：`<d|f>\t<size>\t<mtime>\t<name>\n`（名字含空格安全）。
pub fn format_listing(entries: &[Entry]) -> String {
    let mut out = String::new();
    for e in entries {
        let kind = match e.kind {
            EntryKind::Dir => 'd',
            EntryKind::File => 'f',
        };
        out.push_str(&format!("{kind}\t{}\t{}\t{}\n", e.size, e.mtime_unix, e.name));
    }
    out
}

/// [format_listing] 的逆变换：坏行（字段缺/非数/类型未知）静默跳过。
pub fn parse_listing(body: &str) -> Vec<Entry> {
    body.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(4, '\t');
            let kind = match parts.next()? {
                "d" => EntryKind::Dir,
                "f" => EntryKind::File,
                _ => return None,
            };
            let size = parts.next()?.parse().ok()?;
            let mtime_unix = parts.next()?.parse().ok()?;
            let name = parts.next()?.to_string();
            Some(Entry { name, kind, size, mtime_unix })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_absolute_relative_and_dots() {
        assert_eq!(normalize("/", "/a/b"), Some("/a/b".into()));
        assert_eq!(normalize("/docs", "big.bin"), Some("/docs/big.bin".into()));
        assert_eq!(normalize("/docs", "./x"), Some("/docs/x".into()));
        assert_eq!(normalize("/docs/sub", ".."), Some("/docs".into()));
        assert_eq!(normalize("/docs", ".."), Some("/".into()));
        assert_eq!(normalize("/", ""), Some("/".into()));
    }

    #[test]
    fn normalize_rejects_root_escape() {
        assert_eq!(normalize("/", ".."), None);
        assert_eq!(normalize("/", "/../etc"), None);
        assert_eq!(normalize("/a/b", "../../../x"), None);
    }

    #[test]
    fn listing_roundtrip_and_bad_line_skipped() {
        let entries = vec![
            Entry { name: "a.txt".into(), kind: EntryKind::File, size: 12, mtime_unix: 99 },
            Entry { name: "sub dir".into(), kind: EntryKind::Dir, size: 0, mtime_unix: 7 },
        ];
        let body = format_listing(&entries);
        assert_eq!(parse_listing(&body), entries);
        assert!(parse_listing("x\t1\t2\tbad\nf\tnotnum\t1\tn\n").is_empty());
    }
}
