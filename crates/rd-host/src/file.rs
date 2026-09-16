//! /rd/file/1 文件处理器：按 PeerId 绑定会话 → 隔离根文件服务 → 消息循环。
//! 传输状态机：upload 顺序写（offset 严格校验、断点续传）；download ack 后
//! 立即泵送（M5 限制：下载中不读对端消息，abort 留给 M6，spec 注明）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use p2p::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{ProtocolHandler, ProtocolId};
use rd_fs::{FsError, FsService, TransferHandle};
use rd_wire::io::{recv_file, send_file};
use rd_wire::{FileMsg, XferDir};
use tokio::io;

use crate::sessions::HostSessions;
use crate::HostConfig;

/// /rd/file/1 处理器。
pub struct FileHandler {
    pub sessions: Arc<Mutex<HostSessions>>,
    pub proto: ProtocolId,
    pub fs: FsService,
    pub config: Arc<HostConfig>,
}

#[async_trait::async_trait]
impl ProtocolHandler for FileHandler {
    fn protocol(&self) -> ProtocolId {
        self.proto.clone()
    }

    async fn handle(&self, _stream: BoxedStream) -> io::Result<()> {
        tracing::warn!("rd-host: bare stream without peer identity rejected");
        Ok(())
    }

    async fn handle_inbound(&self, peer: PeerId, mut stream: BoxedStream) -> io::Result<()> {
        let active = match self.sessions.lock() {
            Ok(g) => g.get(&peer).is_some(),
            Err(_) => false,
        };
        if !active {
            tracing::warn!("rd-host: file stream without session from {peer}");
            return Ok(());
        }
        let mut transfers: HashMap<String, TransferHandle> = HashMap::new();
        let idle = std::time::Duration::from_secs(self.config.idle_timeout_secs);
        loop {
            let next = if self.config.idle_timeout_secs > 0 {
                tokio::time::timeout(idle, recv_file(&mut stream)).await
            } else {
                Ok(recv_file(&mut stream).await)
            };
            let msg = match next {
                Ok(Ok(m)) => m,
                Ok(Err(e)) => {
                    tracing::warn!("rd-host: file read error: {e}");
                    break;
                }
                Err(_) => {
                    tracing::warn!("rd-host: file idle timeout (peer {peer})");
                    break;
                }
            };
            let cont = dispatch_file_msg(&self.fs, &mut stream, &mut transfers, msg).await;
            if !cont {
                break;
            }
        }
        Ok(())
    }
}

/// 单消息分发：返回 false 表示会话终止（Close 语义/致命错误）。
async fn dispatch_file_msg(
    fs: &FsService,
    stream: &mut BoxedStream,
    transfers: &mut HashMap<String, TransferHandle>,
    msg: FileMsg,
) -> bool {
    match msg {
        FileMsg::FsList { path } => {
            let result = fs.list(&path).await;
            let (entries, error) = match result {
                Ok(e) => (e, None),
                Err(e) => (Vec::new(), Some(e.to_string())),
            };
            let _ = send_file(
                stream,
                &FileMsg::FsListAck {
                    path,
                    entries,
                    error,
                },
            )
            .await;
        }
        FileMsg::FsStat { path } => {
            let result = fs.stat(&path).await;
            let (entry, error) = match result {
                Ok(e) => (e, None),
                Err(e) => (None, Some(e.to_string())),
            };
            let _ = send_file(stream, &FileMsg::FsStatAck { path, entry, error }).await;
        }
        FileMsg::FsMkdir { path } => {
            let result = fs.mkdir(&path).await;
            send_op_ack(stream, result).await;
        }
        FileMsg::FsRm { path } => {
            let result = fs.rm(&path).await;
            send_op_ack(stream, result).await;
        }
        FileMsg::XferStart {
            id,
            path,
            size,
            direction,
        } => match direction {
            XferDir::Upload => match fs.open_upload(&path, size).await {
                Ok(h) => {
                    let offset = h.offset();
                    transfers.insert(id.clone(), h);
                    let _ = send_file(
                        stream,
                        &FileMsg::XferAck {
                            id,
                            ok: true,
                            reason: None,
                            offset,
                        },
                    )
                    .await;
                }
                Err(e) => {
                    tracing::warn!("rd-host: open upload failed: {e}");
                    let _ = send_file(
                        stream,
                        &FileMsg::XferAck {
                            id,
                            ok: false,
                            reason: Some(e.to_string()),
                            offset: 0,
                        },
                    )
                    .await;
                }
            },
            XferDir::Download => match fs.open_download(&path).await {
                Ok((mut h, size)) => {
                    let _ = send_file(
                        stream,
                        &FileMsg::XferAck {
                            id: id.clone(),
                            ok: true,
                            reason: None,
                            offset: 0,
                        },
                    )
                    .await;
                    pump_download(stream, &id, &mut h, size).await;
                }
                Err(e) => {
                    tracing::warn!("rd-host: open download failed: {e}");
                    let _ = send_file(
                        stream,
                        &FileMsg::XferAck {
                            id,
                            ok: false,
                            reason: Some(e.to_string()),
                            offset: 0,
                        },
                    )
                    .await;
                }
            },
        },
        FileMsg::XferData { id, offset, data } => {
            let raw = match (FileMsg::XferData {
                id: id.clone(),
                offset,
                data,
            })
            .xfer_data_raw()
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("rd-host: xfer data decode failed: {e}");
                    let _ = send_file(stream, &FileMsg::XferEnd { id, ok: false }).await;
                    return true;
                }
            };
            match transfers.get_mut(&id) {
                Some(h) => {
                    if let Err(e) = h.write_at(offset, &raw).await {
                        tracing::warn!("rd-host: write failed: {e}");
                        transfers.remove(&id);
                        let _ = send_file(stream, &FileMsg::XferEnd { id, ok: false }).await;
                    } else if h.done() {
                        transfers.remove(&id);
                        let _ = send_file(stream, &FileMsg::XferEnd { id, ok: true }).await;
                    }
                }
                None => {
                    tracing::warn!("rd-host: xfer data for unknown id {id}");
                    let _ = send_file(stream, &FileMsg::XferEnd { id, ok: false }).await;
                }
            }
        }
        FileMsg::XferEnd { id, ok } => {
            if let Some(h) = transfers.remove(&id) {
                if !ok || !h.done() {
                    tracing::warn!("rd-host: xfer {id} ended abnormally (ok={ok})");
                }
            }
        }
        FileMsg::XferAbort { id } => {
            transfers.remove(&id);
            tracing::info!("rd-host: xfer {id} aborted by peer");
        }
        FileMsg::XferProgress { .. } => {}
        FileMsg::FsListAck { .. }
        | FileMsg::FsStatAck { .. }
        | FileMsg::FsOpAck { .. }
        | FileMsg::XferAck { .. } => {
            tracing::warn!("rd-host: unexpected viewer->host file msg, ignore");
        }
    }
    true
}

/// 下载泵送：ack 后同步发块直到 EOF。
async fn pump_download(stream: &mut BoxedStream, id: &str, h: &mut TransferHandle, size: u64) {
    loop {
        match h.read_next().await {
            Ok(chunk) if !chunk.is_empty() => {
                let msg = match FileMsg::xfer_data(
                    id.to_string(),
                    h.offset() - chunk.len() as u64,
                    &chunk,
                ) {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!("rd-host: download chunk encode failed: {e}");
                        let _ = send_file(
                            stream,
                            &FileMsg::XferEnd {
                                id: id.into(),
                                ok: false,
                            },
                        )
                        .await;
                        return;
                    }
                };
                if let Err(e) = send_file(stream, &msg).await {
                    tracing::warn!("rd-host: download send failed: {e}");
                    return;
                }
            }
            Ok(_) => {
                let _ = send_file(
                    stream,
                    &FileMsg::XferEnd {
                        id: id.into(),
                        ok: true,
                    },
                )
                .await;
                tracing::info!("rd-host: download {id} done ({size} bytes)");
                return;
            }
            Err(e) => {
                tracing::warn!("rd-host: download read failed: {e}");
                let _ = send_file(
                    stream,
                    &FileMsg::XferEnd {
                        id: id.into(),
                        ok: false,
                    },
                )
                .await;
                return;
            }
        }
    }
}

async fn send_op_ack(stream: &mut BoxedStream, result: Result<(), FsError>) {
    let (ok, reason) = match result {
        Ok(()) => (true, None),
        Err(e) => (false, Some(e.to_string())),
    };
    let _ = send_file(stream, &FileMsg::FsOpAck { ok, reason }).await;
}

// 返回 false 语义保留：未来 Close 类消息可在此终止会话。
