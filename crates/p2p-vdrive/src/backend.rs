//! 文件系统后端抽象：服务端可插拔（本地盘 jail / 未来对象存储后端）。
//!
//! 路径一律为服务端虚拟路径：以 `/` 起始，根 = 后端根目录；越狱防护由
//! 具体后端负责。read 返回值长度可为 0（EOF）且 ≤ len；write 语义为
//! 「定位写」，文件不存在时创建（不截断，截断走 truncate/create）。

use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::AsyncRead;

use crate::error::VDriveError;
use crate::wire::{Entry, StatFs, MAX_CHUNK};

pub type FsResult<T> = Result<T, VDriveError>;

#[async_trait]
pub trait FsBackend: Send + Sync {
    async fn statfs(&self) -> FsResult<StatFs>;
    async fn stat(&self, path: &str) -> FsResult<Entry>;
    async fn list(&self, path: &str) -> FsResult<Vec<Entry>>;
    async fn mkdir(&self, path: &str) -> FsResult<()>;
    /// 目录必须为空（NotEmpty 由底座语义统一，不做递归删除）。
    async fn rmdir(&self, path: &str) -> FsResult<()>;
    async fn unlink(&self, path: &str) -> FsResult<()>;
    /// POSIX rename 语义：to 存在则覆盖（目录覆盖要求为空）。
    async fn rename(&self, from: &str, to: &str) -> FsResult<()>;
    async fn truncate(&self, path: &str, size: u64) -> FsResult<()>;
    /// 创建（或截断已存在）空文件，返回创建后的条目。
    async fn create(&self, path: &str) -> FsResult<Entry>;
    /// 定位读：至多 len 字节；len > MAX_CHUNK 由服务端拒绝。
    async fn read(&self, path: &str, offset: u64, len: u32) -> FsResult<Vec<u8>>;
    /// 定位写：文件不存在则创建；返回写入字节数（= data.len()）。
    async fn write(&self, path: &str, offset: u64, data: &[u8]) -> FsResult<u64>;
    /// 整文件流式读（SFTP 同款先例：一次传输一次句柄，禁逐块重开）。
    /// 实现方持有句柄/会话直至读毕；中途失败以短读呈现并留日志。
    async fn open_reader(&self, path: &str) -> FsResult<Box<dyn AsyncRead + Unpin + Send>>;
}

/// 无句柄后端（如远端 RPC 客户端）的缺省 open_reader：chunk 循环泵，
/// 内存占用 = 双份 chunk；短读即 EOF，中途错误留 warn 后截断。
pub fn stream_via_chunks(
    backend: Arc<dyn FsBackend>,
    path: String,
) -> Box<dyn AsyncRead + Unpin + Send> {
    let (mut tx, rx) = tokio::io::duplex(MAX_CHUNK as usize * 2);
    tokio::spawn(async move {
        let mut offset = 0u64;
        loop {
            match backend.read(&path, offset, MAX_CHUNK).await {
                Ok(bytes) if bytes.is_empty() => break,
                Ok(bytes) => {
                    offset += bytes.len() as u64;
                    if tokio::io::AsyncWriteExt::write_all(&mut tx, &bytes)
                        .await
                        .is_err()
                    {
                        break; // 下行端已断开（客户端取消），泵自然结束
                    }
                }
                Err(e) => {
                    tracing::warn!(vpath = %path, "chunk stream read failed mid-stream: {e}");
                    break; // 显式截断：下游以短读感知失败
                }
            }
        }
        use tokio::io::AsyncWriteExt;
        let _ = tx.flush().await;
        drop(tx);
    });
    Box::new(rx)
}

