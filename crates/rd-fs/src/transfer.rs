//! TransferHandle：单传输状态（文件句柄 + 游标 + 声明大小）。
//! 上传顺序写（offset 严格递增校验）；下载顺序读。

use std::path::PathBuf;

use rd_wire::MAX_DATA_RAW_BYTES;

use crate::FsError;

/// 单传输句柄：路径 + 文件 + 游标。
pub struct TransferHandle {
    path: PathBuf,
    file: tokio::fs::File,
    offset: u64,
    total: u64,
}

impl TransferHandle {
    pub(crate) fn new(path: PathBuf, file: tokio::fs::File, offset: u64, total: u64) -> Self {
        Self {
            path,
            file,
            offset,
            total,
        }
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn done(&self) -> bool {
        self.offset >= self.total
    }

    /// 上传块写入：offset 必须等于当前游标（严格顺序，防乱序/重复写）。
    pub async fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<(), FsError> {
        if offset != self.offset {
            return Err(FsError::Transfer(format!(
                "offset mismatch: got {offset}, want {}",
                self.offset
            )));
        }
        if self.offset + data.len() as u64 > self.total {
            return Err(FsError::Transfer("write exceeds declared size".into()));
        }
        tokio::io::AsyncWriteExt::write_all(&mut self.file, data).await?;
        self.offset += data.len() as u64;
        Ok(())
    }

    /// 下载下一块（≤ MAX_DATA_RAW_BYTES）；EOF 返回空 Vec。
    pub async fn read_next(&mut self) -> Result<Vec<u8>, FsError> {
        use tokio::io::AsyncReadExt;
        let mut buf = vec![0u8; MAX_DATA_RAW_BYTES];
        let n = self.file.read(&mut buf).await?;
        if n == 0 {
            return Ok(Vec::new());
        }
        buf.truncate(n);
        self.offset += n as u64;
        Ok(buf)
    }

    /// 传输中路径（错误归因用）。
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}
