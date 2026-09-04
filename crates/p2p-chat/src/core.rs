//! 聊天核心：实时投递、outbox flush、发送状态机与事件（design §6）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use p2p::{Node, NodeEvent, ProtocolId};
use p2p_mux::BoxedStream;
use p2p_protocol::read_frame;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::{broadcast, Mutex as AsyncMutex};

use crate::model::{
    parse_peer_id, sanitize_name, ChatEnvelope, ChatError, ChatEvent, ChatMediaMeta, ChatStatus,
};
use crate::store::Store;
use crate::wire::{self, MediaBegin, MEDIA_BEGIN, MEDIA_CHUNK};
use crate::CHAT_PROTOCOL;

pub(crate) struct ChatCore {
    pub(crate) node: Arc<Node>,
    pub(crate) store: Store,
    pub(crate) events: broadcast::Sender<ChatEvent>,
    /// 每 peer 投递串行锁：send 与 outbox flush 互斥，避免同连接并发开流
    /// （yamux 上游空闲连接二次 open_stream 唤醒丢失缺陷，facade.rs 注释登记）。
    pub(crate) send_locks: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
}

impl ChatCore {
    /// 取该 peer 的投递串行锁（同一时刻每 peer 只有一条在途流）。
    /// 返回 OwnedMutexGuard：持有 Arc，不借用 self，跨 await 安全。
    pub(crate) async fn peer_guard(&self, peer: &str) -> tokio::sync::OwnedMutexGuard<()> {
        let lock = {
            let mut map = self.send_locks.lock().unwrap_or_else(|e| e.into_inner());
            map.entry(peer.to_string())
                .or_insert_with(|| Arc::new(AsyncMutex::new(())))
                .clone()
        };
        lock.lock_owned().await
    }

    /// 重启后把好友簿地址回填节点地址簿（好友可拨性持久化）。
    pub(crate) fn rearm_friend_addrs(&self) -> Result<(), ChatError> {
        for friend in self.store.friends_list()? {
            let peer = match parse_peer_id(&friend.peer_id) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(peer_id = %friend.peer_id, error = %e, "好友 peerId 非法，跳过地址登记");
                    continue;
                }
            };
            for addr in &friend.addrs {
                if let Err(e) = self.node.add_peer_address(peer, addr) {
                    tracing::warn!(peer_id = %friend.peer_id, addr = %addr, error = %e, "好友地址登记失败");
                }
            }
        }
        Ok(())
    }

    pub(crate) fn emit_status(&self, peer: &str, id: &str, status: ChatStatus) {
        let _ = self.events.send(ChatEvent::ChatStatus {
            peer: peer.to_string(),
            message_id: id.to_string(),
            status,
        });
    }

    /// 实时投递：连接（幂等）→ 开流写信封 → 附件分片 → 读 ACK。
    pub(crate) async fn deliver(&self, env: &ChatEnvelope) -> Result<(), ChatError> {
        let peer = parse_peer_id(&env.peer)?;
        self.node
            .connect(peer)
            .await
            .map_err(|e| ChatError::ConnectFailed(format!("连接 {peer} 失败：{e}")))?;
        let proto =
            ProtocolId::new(CHAT_PROTOCOL).map_err(|e| ChatError::Protocol(e.to_string()))?;
        let mut stream = self
            .node
            .new_stream(peer, proto)
            .await
            .map_err(|e| ChatError::SendFailed(format!("开流失败：{e}")))?;
        let wire_env = wire::WireEnvelope::from_outbound(env, self.node.local_peer_id());
        let bytes = serde_json::to_vec(&wire_env).map_err(ChatError::Json)?;
        wire::write_typed(&mut stream, wire::ENVELOPE, &bytes)
            .await
            .map_err(io_err_to_send)?;
        if let Some(media) = &env.media {
            let path = media
                .path
                .as_ref()
                .ok_or_else(|| ChatError::SendFailed("附件路径缺失".into()))?;
            let data = tokio::fs::read(path)
                .await
                .map_err(|e| ChatError::SendFailed(format!("读附件失败：{e}")))?;
            let head = wire::MediaBegin {
                len: data.len() as u64,
                name: media.name.clone(),
                mime: media.mime.clone(),
                kind: env.kind.clone(),
            };
            let hb = serde_json::to_vec(&head).map_err(ChatError::Json)?;
            wire::write_typed(&mut stream, wire::MEDIA_BEGIN, &hb)
                .await
                .map_err(io_err_to_send)?;
            for chunk in data.chunks(wire::CHUNK_LEN) {
                wire::write_typed(&mut stream, wire::MEDIA_CHUNK, chunk)
                    .await
                    .map_err(io_err_to_send)?;
            }
        }
        stream.flush().await.map_err(io_err_to_send)?;
        self.emit_status(&env.peer, &env.id, ChatStatus::Sent);
        let ack = wire::read_ack(&mut stream).await.map_err(io_err_to_send)?;
        if ack.id != env.id {
            return Err(ChatError::SendFailed(format!(
                "ACK id 不匹配：{} ≠ {}",
                ack.id, env.id
            )));
        }
        if !ack.ok {
            return Err(ChatError::SendFailed(format!(
                "对端拒绝：{}",
                ack.reason.as_deref().unwrap_or("")
            )));
        }
        Ok(())
    }

    /// ACK 后置 delivered：outbox 删条目，messages 更新状态。
    pub(crate) fn mark_delivered(&self, peer: &str, env: &ChatEnvelope) -> Result<(), ChatError> {
        self.store.remove_outbox(peer, &env.id)?;
        // 以磁盘 patch 命中与否判定，跨进程交错下不再凭内存视图重复追加（D2）。
        let patched = self
            .store
            .update_message_status(peer, &env.id, ChatStatus::Delivered)?;
        if !patched {
            let mut env = env.clone();
            env.status = ChatStatus::Delivered;
            self.store.append_message(&env)?;
        }
        self.emit_status(peer, &env.id, ChatStatus::Delivered);
        Ok(())
    }

    /// 断流/校验失败：outbox 与 messages 均标记 failed（条目保留待重发）。
    pub(crate) fn mark_failed(&self, peer: &str, env: &ChatEnvelope) -> Result<(), ChatError> {
        self.store
            .set_outbox_status(peer, &env.id, ChatStatus::Failed)?;
        if self.store.has_message(peer, &env.id) {
            self.store
                .update_message_status(peer, &env.id, ChatStatus::Failed)?;
        }
        self.emit_status(peer, &env.id, ChatStatus::Failed);
        Ok(())
    }

    pub(crate) fn status_of(&self, peer: &str, id: &str) -> Result<ChatStatus, ChatError> {
        self.store
            .messages_for(peer)?
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.status)
            .ok_or_else(|| ChatError::NotFound(format!("消息不存在：{id}")))
    }

    /// 对端重连时重发该 peer 全部 outbox 条目；连接失败保持 pending，其余标记 failed。
    /// 持有 peer 串行锁：与 send 并发时等待其完成，避免同连接并发开流（上游缺陷）。
    pub(crate) async fn flush_peer(&self, peer: &str) -> Result<(), ChatError> {
        let _guard = self.peer_guard(peer).await;
        for env in self.store.outbox_for(peer) {
            match self.deliver(&env).await {
                Ok(()) => self.mark_delivered(peer, &env)?,
                Err(ChatError::ConnectFailed(e)) => {
                    tracing::warn!(peer = %peer, id = %env.id, error = %e, "flush 连接失败，保持 pending");
                }
                Err(e) => {
                    tracing::warn!(peer = %peer, id = %env.id, error = %e, "flush 重发失败，保留条目");
                    self.mark_failed(peer, &env)?;
                }
            }
        }
        Ok(())
    }

    /// 入站收媒体：MEDIA_BEGIN 校验 → 逐 MEDIA_CHUNK 写入 tmp 文件 → rename 落盘。
    pub(crate) async fn receive_media(
        &self,
        stream: &mut BoxedStream,
        peer: &str,
        msg_id: &str,
        meta: &ChatMediaMeta,
    ) -> std::io::Result<PathBuf> {
        let frame = read_frame(stream).await?;
        let Some((&kind, payload)) = frame.split_first() else {
            return Err(std::io::Error::other("媒体头帧缺类型头"));
        };
        if kind != MEDIA_BEGIN {
            return Err(std::io::Error::other(format!(
                "期望 MEDIA_BEGIN(0x02)，收到 {kind:#04x}"
            )));
        }
        let head: MediaBegin = serde_json::from_slice(payload)
            .map_err(|e| std::io::Error::other(format!("媒体头 JSON 非法：{e}")))?;
        if head.len != meta.size {
            return Err(std::io::Error::other(format!(
                "媒体长度不一致：头 {} ≠ 信封 {}",
                head.len, meta.size
            )));
        }
        let dir = self.store.media_peer_dir(peer)?;
        let final_path = dir.join(format!("{msg_id}_{}", sanitize_name(&meta.name)));
        let tmp = dir.join(format!(".tmp-{msg_id}-{}", std::process::id()));
        let mut file = fs::File::create(&tmp).await?;
        let mut received: u64 = 0;
        while received < head.len {
            let frame = read_frame(stream).await?;
            let Some((&kind, payload)) = frame.split_first() else {
                let _ = fs::remove_file(&tmp).await;
                return Err(std::io::Error::other("媒体分片缺类型头"));
            };
            if kind != MEDIA_CHUNK {
                let _ = fs::remove_file(&tmp).await;
                return Err(std::io::Error::other(format!(
                    "期望 MEDIA_CHUNK(0x03)，收到 {kind:#04x}"
                )));
            }
            received += payload.len() as u64;
            if received > head.len {
                let _ = fs::remove_file(&tmp).await;
                return Err(std::io::Error::other("媒体超过声明长度，断流"));
            }
            file.write_all(payload).await?;
        }
        file.flush().await?;
        drop(file);
        fs::rename(&tmp, &final_path).await?;
        Ok(final_path)
    }
}

/// 监听 PeerConnected：触发该 peer 的 outbox flush（离线投递语义 §6.2）。
pub(crate) fn spawn_outbox_task(core: Arc<ChatCore>) {
    tokio::spawn(async move {
        let mut rx = core.node.events();
        loop {
            match rx.recv().await {
                Ok(NodeEvent::PeerConnected { peer }) => {
                    let peer_s = peer.to_string();
                    if let Err(e) = core.flush_peer(&peer_s).await {
                        tracing::warn!(%peer, error = %e, "outbox flush 失败");
                    }
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

fn io_err_to_send(e: std::io::Error) -> ChatError {
    ChatError::SendFailed(format!("流 IO 失败：{e}"))
}
