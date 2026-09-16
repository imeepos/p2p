//! viewer 文件通道：懒开 /rd/file/1 流，单飞操作（list/stat/mkdir/rm/upload/download）。

use std::path::Path;
use std::sync::Arc;

use p2p::Node;
use p2p_mux::BoxedStream;
use rd_wire::io::{recv_file, send_file};
use rd_wire::{Entry, FileMsg, XferDir};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio::sync::Mutex;

use crate::ViewerError;

/// 文件通道：控制/视频通道之外的独立 /rd/file/1 流，首次操作时懒开。
pub struct FileChannel {
    node: Arc<Node>,
    peer: p2p::PeerId,
    stream: Mutex<Option<BoxedStream>>,
}

impl FileChannel {
    pub fn new(node: Arc<Node>, peer: p2p::PeerId) -> Self {
        Self {
            node,
            peer,
            stream: Mutex::new(None),
        }
    }

    async fn lock_stream(
        &self,
    ) -> Result<tokio::sync::MutexGuard<'_, Option<BoxedStream>>, ViewerError> {
        let mut g = self.stream.lock().await;
        if g.is_none() {
            let s = self
                .node
                .new_stream(self.peer, rd_wire::file_protocol_id()?)
                .await?;
            *g = Some(s);
        }
        Ok(g)
    }

    /// 目录浏览。
    pub async fn list(&self, path: &str) -> Result<Vec<Entry>, ViewerError> {
        let mut g = self.lock_stream().await?;
        let stream = g.as_mut().ok_or(ViewerError::Closed)?;
        send_file(stream, &FileMsg::FsList { path: path.into() }).await?;
        loop {
            if let FileMsg::FsListAck { entries, error, .. } = recv_file(stream).await? {
                return match error {
                    Some(e) => Err(ViewerError::Decode(e)),
                    None => Ok(entries),
                };
            }
        }
    }

    /// 单条目 stat（None = 不存在）。
    pub async fn stat(&self, path: &str) -> Result<Option<Entry>, ViewerError> {
        let mut g = self.lock_stream().await?;
        let stream = g.as_mut().ok_or(ViewerError::Closed)?;
        send_file(stream, &FileMsg::FsStat { path: path.into() }).await?;
        loop {
            if let FileMsg::FsStatAck { entry, error, .. } = recv_file(stream).await? {
                return match error {
                    Some(e) => Err(ViewerError::Decode(e)),
                    None => Ok(entry),
                };
            }
        }
    }

    /// 建目录。
    pub async fn mkdir(&self, path: &str) -> Result<(), ViewerError> {
        self.op_ack(FileMsg::FsMkdir { path: path.into() }).await
    }

    /// 删除。
    pub async fn rm(&self, path: &str) -> Result<(), ViewerError> {
        self.op_ack(FileMsg::FsRm { path: path.into() }).await
    }

    async fn op_ack(&self, msg: FileMsg) -> Result<(), ViewerError> {
        let mut g = self.lock_stream().await?;
        let stream = g.as_mut().ok_or(ViewerError::Closed)?;
        send_file(stream, &msg).await?;
        loop {
            if let FileMsg::FsOpAck { ok, reason } = recv_file(stream).await? {
                return if ok {
                    Ok(())
                } else {
                    Err(ViewerError::Rejected(
                        reason.unwrap_or_else(|| "op failed".into()),
                    ))
                };
            }
        }
    }

    /// 上传本地文件到 host 隔离根内的相对路径。
    pub async fn upload(
        &self,
        local: &Path,
        remote: &str,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<(), ViewerError> {
        let size = tokio::fs::metadata(local).await?.len();
        let mut file = tokio::fs::File::open(local).await?;
        let mut g = self.lock_stream().await?;
        let stream = g.as_mut().ok_or(ViewerError::Closed)?;
        let id = uuid::Uuid::new_v4().simple().to_string();
        send_file(
            stream,
            &FileMsg::XferStart {
                id: id.clone(),
                path: remote.into(),
                size,
                direction: XferDir::Upload,
            },
        )
        .await?;
        let start = await_xfer_ack(stream, &id).await?;
        if start > 0 {
            file.seek(SeekFrom::Start(start)).await?;
        }
        let mut done = start;
        let mut buf = vec![0u8; rd_wire::MAX_DATA_RAW_BYTES];
        loop {
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            let chunk = buf[..n].to_vec();
            send_file(stream, &FileMsg::xfer_data(id.clone(), done, &chunk)?).await?;
            done += n as u64;
            progress(done, size);
        }
        send_file(
            stream,
            &FileMsg::XferEnd {
                id: id.clone(),
                ok: true,
            },
        )
        .await?;
        // 等 host 完传确认（写盘完成后才返回，供调用方安全读远端文件）
        loop {
            if let FileMsg::XferEnd { id: mid, ok } = recv_file(stream).await? {
                if mid == id {
                    return if ok {
                        Ok(())
                    } else {
                        Err(ViewerError::Rejected("host reported upload failure".into()))
                    };
                }
            }
        }
    }

    /// 下载 host 文件到本地路径。
    pub async fn download(
        &self,
        remote: &str,
        local: &Path,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<(), ViewerError> {
        let mut g = self.lock_stream().await?;
        let stream = g.as_mut().ok_or(ViewerError::Closed)?;
        let id = uuid::Uuid::new_v4().simple().to_string();
        send_file(
            stream,
            &FileMsg::XferStart {
                id: id.clone(),
                path: remote.into(),
                size: 0,
                direction: XferDir::Download,
            },
        )
        .await?;
        let _start = await_xfer_ack(stream, &id).await?;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(local)
            .await?;
        let mut done = 0u64;
        loop {
            match recv_file(stream).await? {
                FileMsg::XferData {
                    id: mid,
                    offset,
                    data,
                } if mid == id => {
                    let raw = (FileMsg::XferData {
                        id: mid,
                        offset,
                        data,
                    })
                    .xfer_data_raw()?;
                    if offset != done {
                        file.seek(SeekFrom::Start(offset)).await?;
                    }
                    file.write_all(&raw).await?;
                    done = offset + raw.len() as u64;
                    progress(done, done);
                }
                FileMsg::XferEnd { id: mid, ok } if mid == id => {
                    return if ok {
                        file.flush().await?;
                        Ok(())
                    } else {
                        Err(ViewerError::Decode("download failed on host".into()))
                    };
                }
                FileMsg::XferAbort { id: mid } if mid == id => {
                    return Err(ViewerError::Aborted("download aborted by host".into()));
                }
                _ => {}
            }
        }
    }
}

/// 等待 XferStart 的 XferAck（严格 id 匹配；拒绝带 reason 上抛）。
async fn await_xfer_ack(stream: &mut BoxedStream, id: &str) -> Result<u64, ViewerError> {
    loop {
        match recv_file(stream).await? {
            FileMsg::XferAck {
                id: mid,
                ok,
                reason,
                offset,
            } if mid == id => {
                return if ok {
                    Ok(offset)
                } else {
                    Err(ViewerError::Rejected(
                        reason.unwrap_or_else(|| "xfer rejected".into()),
                    ))
                };
            }
            _ => {}
        }
    }
}
