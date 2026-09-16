//! LocalFs 的 FsBackend 实现（操作面）。

use std::path::Path;

use async_trait::async_trait;
use tokio::fs;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use super::{entry_of, name_of, LocalFs};
use crate::backend::FsBackend;
use crate::backend::FsResult;
use crate::error::{ErrorKind, VDriveError};
use crate::wire::{Entry, StatFs};

#[async_trait]
impl FsBackend for LocalFs {
    async fn statfs(&self) -> FsResult<StatFs> {
        // libc::statvfs（lfs-core 同款）：真容量喂给 WebDAV RFC 4331 quota。
        // 纯内存元数据读取（微秒级），不值得 spawn_blocking。
        #[cfg(unix)]
        {
            let cpath = std::ffi::CString::new(self.root.as_os_str().as_encoded_bytes())
                .map_err(|_| VDriveError::new(ErrorKind::InvalidPath, "root path has NUL"))?;
            let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
            let rc = unsafe { libc::statvfs(cpath.as_ptr(), &mut st) };
            if rc != 0 {
                return Err(VDriveError::from(std::io::Error::last_os_error()));
            }
            let bs = st.f_frsize as u64;
            Ok(StatFs {
                total_bytes: st.f_blocks as u64 * bs,
                free_bytes: st.f_bavail as u64 * bs,
            })
        }
        #[cfg(not(unix))]
        {
            Ok(StatFs::default())
        }
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
    async fn statfs_reports_real_capacity() {
        let tmp = tempfile::tempdir().unwrap();
        let f = LocalFs::open(tmp.path()).await.unwrap();
        let s = f.statfs().await.unwrap();
        assert!(s.total_bytes > 0, "宿主容量必须为真值: {s:?}");
        assert!(s.free_bytes > 0);
        assert!(s.free_bytes <= s.total_bytes);
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
