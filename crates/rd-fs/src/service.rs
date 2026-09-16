//! FsService：隔离根目录内的浏览/建删/传输登记。

use std::path::{Path, PathBuf};

use rd_wire::{sanitize_rel_path, Entry, FsKind};
use thiserror::Error;

use crate::transfer::TransferHandle;

/// 文件服务失败：显式可观测。
#[derive(Debug, Error)]
pub enum FsError {
    #[error("unsafe path: {0}")]
    UnsafePath(String),
    #[error("path escapes jail root: {0}")]
    Escape(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("transfer: {0}")]
    Transfer(String),
}

/// 隔离根文件服务：所有路径操作限制在 root 内。
pub struct FsService {
    root: PathBuf,
}

impl FsService {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 相对 POSIX 路径 → 狱内绝对路径（wire 卫生 + 符号链接逃逸双重校验）。
    /// 空串 = 根目录（列表根用）；不存在段按普通目录处理（创建流程会建父链）。
    pub fn resolve(&self, rel: &str) -> Result<PathBuf, FsError> {
        let clean = if rel.is_empty() {
            String::new()
        } else {
            sanitize_rel_path(rel).map_err(|e| FsError::UnsafePath(e.to_string()))?
        };
        let root = self.root.canonicalize().map_err(FsError::Io)?;
        let candidate = root.join(&clean);
        // 最深已存在祖先 canonicalize 后必须是 root 后代（防既有符号链接指出去）。
        let mut probe = candidate.as_path();
        let mut suffix: Vec<std::ffi::OsString> = Vec::new();
        while !probe.exists() {
            let Some(name) = probe.file_name() else { break };
            suffix.push(name.to_os_string());
            let Some(parent) = probe.parent() else { break };
            probe = parent;
        }
        let real = probe
            .canonicalize()
            .map_err(|_| FsError::NotFound(rel.into()))?;
        if !real.starts_with(&root) {
            return Err(FsError::Escape(rel.into()));
        }
        Ok(candidate)
    }

    /// 目录浏览：返回排序后的条目。
    pub async fn list(&self, rel: &str) -> Result<Vec<Entry>, FsError> {
        let dir = self.resolve(rel)?;
        let mut rd = tokio::fs::read_dir(&dir)
            .await
            .map_err(|_| FsError::NotFound(rel.into()))?;
        let mut entries = Vec::new();
        while let Some(e) = rd.next_entry().await? {
            let name = e.file_name().to_string_lossy().into_owned();
            let meta = e.metadata().await?;
            entries.push(Entry {
                name,
                kind: if meta.is_dir() {
                    FsKind::Dir
                } else if meta.is_file() {
                    FsKind::File
                } else {
                    FsKind::Other
                },
                size: meta.len(),
                mtime: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    /// 单条目 stat；不存在返回 Ok(None)。
    pub async fn stat(&self, rel: &str) -> Result<Option<Entry>, FsError> {
        let path = self.resolve(rel)?;
        let meta = match tokio::fs::metadata(&path).await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| rel.to_string());
        Ok(Some(Entry {
            name,
            kind: if meta.is_dir() {
                FsKind::Dir
            } else if meta.is_file() {
                FsKind::File
            } else {
                FsKind::Other
            },
            size: meta.len(),
            mtime: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }))
    }

    /// 建目录（含中间路径）。
    pub async fn mkdir(&self, rel: &str) -> Result<(), FsError> {
        let path = self.resolve(rel)?;
        tokio::fs::create_dir_all(&path).await?;
        Ok(())
    }

    /// 删除：目录递归删，文件直删（M5 不做回收站语义，spec 注明）。
    pub async fn rm(&self, rel: &str) -> Result<(), FsError> {
        let path = self.resolve(rel)?;
        let meta = tokio::fs::metadata(&path)
            .await
            .map_err(|_| FsError::NotFound(rel.into()))?;
        if meta.is_dir() {
            tokio::fs::remove_dir_all(&path).await?;
        } else {
            tokio::fs::remove_file(&path).await?;
        }
        Ok(())
    }

    /// 登记上传：已存在且 ≤ 声明大小的部分文件续写（断点续传），否则截断重建。
    pub async fn open_upload(&self, rel: &str, size: u64) -> Result<TransferHandle, FsError> {
        let path = self.resolve(rel)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let existing = tokio::fs::metadata(&path).await.ok();
        let offset = match existing {
            Some(m) if m.is_file() && m.len() <= size => m.len(),
            _ => {
                tokio::fs::remove_file(&path).await.ok();
                0
            }
        };
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        Ok(TransferHandle::new(path, file, offset, size))
    }

    /// 登记下载：打开文件读取；返回句柄与总大小。
    pub async fn open_download(&self, rel: &str) -> Result<(TransferHandle, u64), FsError> {
        let path = self.resolve(rel)?;
        let meta = tokio::fs::metadata(&path)
            .await
            .map_err(|_| FsError::NotFound(rel.into()))?;
        if !meta.is_file() {
            return Err(FsError::Transfer(format!("not a file: {rel}")));
        }
        let file = tokio::fs::File::open(&path).await?;
        let size = meta.len();
        Ok((TransferHandle::new(path, file, 0, size), size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("rd-fs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[tokio::test]
    async fn resolve_rejects_escape() {
        let svc = FsService::new(temp_root("esc"));
        assert!(svc.resolve("../x").is_err());
        assert!(svc.resolve("/etc/passwd").is_err());
        assert!(svc.resolve("a/../../b").is_err());
        assert!(svc.resolve("").is_ok(), "空串 = 根目录");
        assert!(svc.resolve("ok/file.txt").is_ok());
    }

    #[tokio::test]
    async fn list_mkdir_rm_roundtrip() {
        let root = temp_root("lrm");
        std::fs::write(root.join("a.txt"), "hello").unwrap();
        let svc = FsService::new(root);
        let entries = svc.list("").await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "a.txt");
        svc.mkdir("sub/dir").await.unwrap();
        let entries = svc.list("").await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "a.txt");
        assert_eq!(entries[1].name, "sub");
        svc.rm("sub").await.unwrap();
        assert_eq!(svc.list("").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn upload_resume_from_partial() {
        let root = temp_root("resume");
        std::fs::write(root.join("f.bin"), b"0123456789").unwrap();
        let svc = FsService::new(root);
        let h = svc.open_upload("f.bin", 20).await.unwrap();
        assert_eq!(h.offset(), 10, "部分文件应续写");
        let h2 = svc.open_upload("g.bin", 100).await.unwrap();
        assert_eq!(h2.offset(), 0, "新文件从头写");
    }
}
