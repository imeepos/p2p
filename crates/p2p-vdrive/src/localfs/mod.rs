//! LocalFs：本地目录监狱后端（操作面实现见 [ops]）。
//!
//! 双闸越狱防护：`normalize` 先折叠 `.`/`..` 与空段（越出根即拒），
//! `resolve` 再对目标/最近存在祖先 canonicalize 并校验前缀（防 symlink
//! 与大小写路径逃逸）。

use std::path::{Path, PathBuf};

use tokio::fs;

use crate::backend::FsResult;
use crate::error::{ErrorKind, VDriveError};
use crate::wire::{Entry, EntryKind};

pub struct LocalFs {
    pub(super) root: PathBuf,
    pub(super) root_canon: PathBuf,
}

impl LocalFs {
    /// 根目录必须已存在且为目录（装配期显式失败，禁静默服务空根）。
    pub async fn open(root: impl Into<PathBuf>) -> std::io::Result<Self> {
        let root = root.into();
        let meta = fs::metadata(&root).await?;
        if !meta.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("vdrive root is not a directory: {}", root.display()),
            ));
        }
        let root = root.canonicalize().unwrap_or(root);
        let root_canon = fs::canonicalize(&root).await?;
        Ok(Self { root, root_canon })
    }

    /// 虚拟路径 → 监狱内绝对路径；越狱返回 InvalidPath。
    pub fn resolve(&self, vpath: &str) -> FsResult<PathBuf> {
        let rel = normalize_vpath(vpath).ok_or_else(|| {
            VDriveError::new(
                ErrorKind::InvalidPath,
                format!("path escapes root: {vpath}"),
            )
        })?;
        let target = if rel == "/" {
            self.root.clone()
        } else {
            self.root.join(&rel[1..])
        };
        // 对目标或最近存在祖先 canonicalize 后校验仍在根内。
        let mut probe = target.as_path();
        loop {
            if probe.as_os_str().is_empty() {
                break;
            }
            if probe.is_symlink() || probe.exists() {
                break;
            }
            probe = probe.parent().unwrap_or(Path::new(""));
        }
        if let Ok(canon) = std::fs::canonicalize(probe) {
            if !canon.starts_with(&self.root_canon) {
                return Err(VDriveError::new(
                    ErrorKind::InvalidPath,
                    format!("resolved path escapes root: {vpath}"),
                ));
            }
        }
        Ok(target)
    }

    fn parent_of(&self, vpath: &str) -> FsResult<PathBuf> {
        let path = self.resolve(vpath)?;
        Ok(path.parent().unwrap_or(Path::new("")).to_path_buf())
    }
}

mod ops;

pub use crate::path::normalize_vpath;

pub(super) fn entry_of(name: String, meta: &std::fs::Metadata) -> Entry {
    let kind = if meta.is_dir() {
        EntryKind::Dir
    } else {
        EntryKind::File
    };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ctime = meta
        .created()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(mtime);
    Entry {
        name,
        kind,
        size: meta.len(),
        mtime,
        ctime,
    }
}

pub(super) fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
