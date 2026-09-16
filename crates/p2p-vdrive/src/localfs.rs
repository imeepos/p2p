//! LocalFs：本地目录监狱后端。
//!
//! 双闸越狱防护：`normalize` 先折叠 `.`/`..` 与空段（越出根即拒），
//! `resolve` 再对目标/最近存在祖先 canonicalize 并校验前缀（防 symlink
//! 与大小写路径逃逸）。

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::fs;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::backend::{FsBackend, FsResult};
use crate::error::{ErrorKind, VDriveError};
use crate::wire::{Entry, EntryKind, StatFs};

pub struct LocalFs {
    root: PathBuf,
    root_canon: PathBuf,
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

pub use crate::path::normalize_vpath;

fn entry_of(name: String, meta: &std::fs::Metadata) -> Entry {
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

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[async_trait]
impl FsBackend for LocalFs {
    async fn statfs(&self) -> FsResult<StatFs> {
        // M1：容量不报告（0 = unknown），映射为 WebDAV 不含配额属性。
        Ok(StatFs::default())
    }

    async fn stat(&self, path: &str) -> FsResult<Entry> {
        let target = self.resolve(path)?;
        let meta = fs::metadata(&target).await?;
        // 根条目名字恒空（WebDAV href = "/"），非根取末段。
        let name = if target == self.root {
            String::new()
        } else {
            name_of(&target)
        };
        Ok(entry_of(name, &meta))
    }

    async fn list(&self, path: &str) -> FsResult<Vec<Entry>> {
        let target = self.resolve(path)?;
        let mut reader = fs::read_dir(&target).await?;
        let mut out = Vec::new();
        while let Some(item) = reader.next_entry().await? {
            let name = name_of(&item.path());
            let entry = entry_of(name, &item.metadata().await?);
            out.push(entry);
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    async fn mkdir(&self, path: &str) -> FsResult<()> {
        let target = self.resolve(path)?;
        fs::create_dir(&target).await?;
        Ok(())
    }

    async fn rmdir(&self, path: &str) -> FsResult<()> {
        let target = self.resolve(path)?;
        if target == self.root {
            return Err(VDriveError::new(
                ErrorKind::PermissionDenied,
                "cannot remove drive root",
            ));
        }
        fs::remove_dir(&target).await?;
        Ok(())
    }

    async fn unlink(&self, path: &str) -> FsResult<()> {
        let target = self.resolve(path)?;
        let meta = fs::metadata(&target).await?;
        if meta.is_dir() {
            return Err(VDriveError::new(ErrorKind::NotDir, "is a directory"));
        }
        fs::remove_file(&target).await?;
        Ok(())
    }

    async fn rename(&self, from: &str, to: &str) -> FsResult<()> {
        let src = self.resolve(from)?;
        let dst = self.resolve(to)?;
        // 目标父目录必须已存在（与 WebDAV MOVE 409 语义对齐，禁隐式建父）。
        if !self.parent_of(to)?.is_dir() {
            return Err(VDriveError::new(
                ErrorKind::NotFound,
                "destination parent missing",
            ));
        }
        fs::rename(&src, &dst).await?;
        Ok(())
    }

    async fn truncate(&self, path: &str, size: u64) -> FsResult<()> {
        let target = self.resolve(path)?;
        let f = fs::OpenOptions::new()
            .write(true)
            .open(&target)
            .await
            .map_err(|e| attach_not_dir(e, &target))?;
        f.set_len(size).await?;
        Ok(())
    }

    async fn create(&self, path: &str) -> FsResult<Entry> {
        if !self.parent_of(path)?.is_dir() {
            return Err(VDriveError::new(
                ErrorKind::NotFound,
                "parent directory missing",
            ));
        }
        let target = self.resolve(path)?;
        let f = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&target)
            .await
            .map_err(|e| attach_not_dir(e, &target))?;
        drop(f);
        let meta = fs::metadata(&target).await?;
        Ok(entry_of(name_of(&target), &meta))
    }

    async fn read(&self, path: &str, offset: u64, len: u32) -> FsResult<Vec<u8>> {
        let target = self.resolve(path)?;
        let mut f = fs::File::open(&target)
            .await
            .map_err(|e| attach_not_dir(e, &target))?;
        let meta = f.metadata().await?;
        if meta.is_dir() {
            return Err(VDriveError::new(ErrorKind::NotDir, "is a directory"));
        }
        if offset >= meta.len() {
            return Ok(Vec::new());
        }
        f.seek(std::io::SeekFrom::Start(offset)).await?;
        let cap = (meta.len() - offset).min(len as u64) as usize;
        let mut buf = vec![0u8; cap];
        let mut filled = 0;
        while filled < cap {
            let n = f.read(&mut buf[filled..]).await?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        buf.truncate(filled);
        Ok(buf)
    }

    async fn write(&self, path: &str, offset: u64, data: &[u8]) -> FsResult<u64> {
        if !self.parent_of(path)?.is_dir() {
            return Err(VDriveError::new(
                ErrorKind::NotFound,
                "parent directory missing",
            ));
        }
        let target = self.resolve(path)?;
        let mut f = fs::OpenOptions::new()
            .create(true)
            .truncate(false) // 定位写：保留未覆盖尾段（截断走 truncate/create）
            .write(true)
            .open(&target)
            .await
            .map_err(|e| attach_not_dir(e, &target))?;
        f.seek(std::io::SeekFrom::Start(offset)).await?;
        f.write_all(data).await?;
        f.flush().await?;
        Ok(data.len() as u64)
    }

    async fn open_reader(&self, path: &str) -> FsResult<Box<dyn AsyncRead + Unpin + Send>> {
        let target = self.resolve(path)?;
        let f = fs::File::open(&target)
            .await
            .map_err(|e| attach_not_dir(e, &target))?;
        if f.metadata().await?.is_dir() {
            return Err(VDriveError::new(ErrorKind::NotDir, "is a directory"));
        }
        Ok(Box::new(f))
    }
}

/// 目录路径上的文件操作统一转 NotDir（而非底层 io 错误），供 HTTP 405 映射。
fn attach_not_dir(e: std::io::Error, path: &Path) -> std::io::Error {
    if path.is_dir() {
        return std::io::Error::new(std::io::ErrorKind::NotADirectory, "is a directory");
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn open_rejects_missing_root() {
        assert!(LocalFs::open("/definitely/not/exists").await.is_err());
    }

    #[tokio::test]
    async fn open_reader_streams_whole_file_once() {
        let tmp = tempfile::tempdir().unwrap();
        let f = LocalFs::open(tmp.path()).await.unwrap();
        fs::create_dir(tmp.path().join("d")).await.unwrap();
        fs::write(tmp.path().join("d/big.bin"), vec![7u8; 600_000])
            .await
            .unwrap();
        let mut reader = f.open_reader("/d/big.bin").await.unwrap();
        use tokio::io::AsyncReadExt;
        let mut got = Vec::new();
        reader.read_to_end(&mut got).await.unwrap();
        assert_eq!(got.len(), 600_000);
        assert!(got.iter().all(|b| *b == 7));
        // 目录读体必须 NotDir（HTTP 405 映射面）。
        let err = match f.open_reader("/d").await {
            Err(e) => e,
            Ok(_) => panic!("目录读体应被拒绝"),
        };
        assert_eq!(err.kind, ErrorKind::NotDir);
    }
}
