//! 本地盘后端：根目录监狱。
//!
//! 越狱双闸：① `normalize` 后组件层拒绝 `..`（vfs::normalize 已拒，这里
//! 再按空段过滤重建相对路径）；② 打开已存在路径时 canonicalize 并校验
//! 仍在根内（符号链接逃逸）；新建路径校验其父目录在根内。

use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::fs;

use crate::vfs::{Entry, EntryKind, FileSystem};

pub struct LocalFs {
    root: PathBuf,
}

impl LocalFs {
    /// 根目录 canonicalize 后固定为监狱边界。
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        Ok(Self { root: std::fs::canonicalize(root.into())? })
    }

    fn jail_err() -> io::Error {
        io::Error::new(io::ErrorKind::PermissionDenied, "path escapes server root")
    }

    /// 虚拟路径 → 磁盘路径；存在路径 canonicalize 反符号链接逃逸。
    fn resolve(&self, vpath: &str) -> io::Result<PathBuf> {
        let target = self.root.join(relative_parts(vpath));
        if let Ok(real) = std::fs::canonicalize(&target) {
            if !real.starts_with(&self.root) {
                return Err(Self::jail_err());
            }
        }
        Ok(target)
    }

    /// 写路径解析：父目录必须已存在且在根内（防新建符号链接越界）。
    fn resolve_for_write(&self, vpath: &str) -> io::Result<PathBuf> {
        let target = self.root.join(relative_parts(vpath));
        let parent = target
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no parent"))?;
        let real_parent = std::fs::canonicalize(parent)?;
        if !real_parent.starts_with(&self.root) {
            return Err(Self::jail_err());
        }
        Ok(target)
    }
}

fn relative_parts(vpath: &str) -> PathBuf {
    vpath
        .split('/')
        .filter(|seg| !seg.is_empty() && *seg != "." && *seg != "..")
        .collect::<PathBuf>()
}

fn to_entry(name: String, meta: &std::fs::Metadata) -> Entry {
    let kind = if meta.is_dir() { EntryKind::Dir } else { EntryKind::File };
    let mtime_unix = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Entry { name, kind, size: meta.len(), mtime_unix }
}

async fn dir_entries(path: &Path) -> io::Result<Vec<Entry>> {
    let mut out = Vec::new();
    let mut rd = fs::read_dir(path).await?;
    while let Some(item) = rd.next_entry().await? {
        let meta = item.metadata().await?;
        out.push(to_entry(item.file_name().to_string_lossy().into_owned(), &meta));
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

async fn entry_meta(path: &Path, name: &str) -> io::Result<Entry> {
    let meta = fs::metadata(path).await?;
    Ok(to_entry(name.to_string(), &meta))
}

#[async_trait]
impl FileSystem for LocalFs {
    async fn list(&self, vpath: &str) -> io::Result<Vec<Entry>> {
        let dir = self.resolve(vpath)?;
        if !dir.exists() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "no such directory"));
        }
        dir_entries(&dir).await
    }

    async fn metadata(&self, vpath: &str) -> io::Result<Entry> {
        let target = self.resolve(vpath)?;
        let name = target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/".to_string());
        entry_meta(&target, &name).await
    }

    async fn mkdir(&self, vpath: &str) -> io::Result<()> {
        fs::create_dir(self.resolve_for_write(vpath)?).await
    }

    async fn remove_dir(&self, vpath: &str) -> io::Result<()> {
        fs::remove_dir(self.resolve(vpath)?).await
    }

    async fn remove_file(&self, vpath: &str) -> io::Result<()> {
        fs::remove_file(self.resolve(vpath)?).await
    }

    async fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let src = self.resolve(from)?;
        if std::fs::canonicalize(&src).is_err() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "rename source missing"));
        }
        fs::rename(src, self.resolve_for_write(to)?).await
    }

    async fn reader(&self, vpath: &str) -> io::Result<Box<dyn AsyncRead + Unpin + Send>> {
        let path = self.resolve(vpath)?;
        let file = fs::File::open(&path).await?;
        Ok(Box::new(file))
    }

    async fn writer(&self, vpath: &str, append: bool) -> io::Result<Box<dyn AsyncWrite + Unpin + Send>> {
        let path = self.resolve_for_write(vpath)?;
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&path)
            .await?;
        Ok(Box::new(file))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "p2p-ftp-{tag}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn write_read_roundtrip_inside_root() {
        let root = scratch("rw");
        let fs = LocalFs::open(&root).unwrap();
        fs.mkdir("/sub").await.unwrap();
        let mut w = fs.writer("/sub/a.txt", false).await.unwrap();
        tokio::io::AsyncWriteExt::write_all(&mut w, b"hello").await.unwrap();

        let mut buf = Vec::new();
        let mut r = fs.reader("/sub/a.txt").await.unwrap();
        tokio::io::AsyncReadExt::read_to_end(&mut r, &mut buf).await.unwrap();
        assert_eq!(buf, b"hello");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn escape_paths_are_rejected() {
        let root = scratch("jail");
        // 与根平级的对照目录：写入绝不允许落到根外
        let outside = std::env::temp_dir().join(format!("p2p-ftp-outside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&outside);
        std::fs::create_dir_all(&outside).unwrap();
        let fs = LocalFs::open(&root).unwrap();

        // chroot 语义：越出根的 .. 折叠回根内（会话层 normalize 已先行 550）
        fs.writer("/../evil.txt", false).await.unwrap();
        assert!(root.join("evil.txt").exists());
        assert!(!outside.join("evil.txt").exists(), "写入不得落在根外");
        assert!(fs.reader("/../missing").await.is_err());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlink_escape_is_rejected() {
        let root = scratch("symlink");
        let outside = scratch("outside");
        std::fs::write(outside.join("x"), b"leak").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("door")).unwrap();
        let fs = LocalFs::open(&root).unwrap();

        assert!(fs.reader("/door/x").await.is_err());
        assert!(fs.list("/door").await.is_err());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }
}
