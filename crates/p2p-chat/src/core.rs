//! 聊天核心：实时投递、outbox flush、发送状态机与事件（design §6）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use p2p::{Node, ProtocolId};
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

/// 单次投递内 ACK 等待上限：死连接上 read_ack 无界等待是演练 D1 卡死的直接面。
const ACK_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct ChatCore {
    pub(crate) node: Arc<Node>,
    pub(crate) store: Store,
    pub(crate) events: broadcast::Sender<ChatEvent>,
    /// 每 peer 投递串行锁：send 与 outbox flush 互斥，避免同连接并发开流
    /// （yamux 上游空闲连接二次 open_stream 唤醒丢失缺陷，facade.rs 注释登记）。
    pub(crate) send_locks: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
    /// 已给过重投机会的 failed 条目（peer,id）：每进程一次机会，二次即死信（outbox.rs）。
    pub(crate) flush_tried: Mutex<HashMap<(String, String), ()>>,
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
    /// 传输类失败（流 IO/ACK 超时/开流失败）强拆本进程到该 peer 的连接重拨再试一次：
    /// 多进程共享身份时连接池可能复用正在死亡进程的连接（跨机演练 D1）。
    /// 协议类失败（对端拒绝/ACK 不匹配）确定性复现，不重试。连接失败保持 pending 语义。
    pub(crate) async fn deliver(&self, env: &ChatEnvelope) -> Result<(), ChatError> {
        let pid = parse_peer_id(&env.peer)?;
        match self.deliver_stream(pid, env).await {
            Ok(()) => Ok(()),
            Err(ChatError::ConnectFailed(e)) => Err(ChatError::ConnectFailed(e)),
            Err(first @ ChatError::StreamFailed(_)) => {
                tracing::warn!(
                    peer = %env.peer,
                    id = %env.id,
                    error = %first,
                    "投递传输失败，强拆重拨重试一次"
                );
                self.node.disconnect(&pid);
                self.node.connect(pid).await.map_err(|e| {
                    ChatError::ConnectFailed(format!("连接 {} 失败：{e}", env.peer))
                })?;
                self.deliver_stream(pid, env).await
            }
            Err(e) => Err(e),
        }
    }

    /// 单次投递尝试（连接 → 开流 → 写 → 读 ACK）。
    async fn deliver_stream(
        &self,
        pid: p2p_identity::PeerId,
        env: &ChatEnvelope,
    ) -> Result<(), ChatError> {
        self.node
            .connect(pid)
            .await
            .map_err(|e| ChatError::ConnectFailed(format!("连接 {} 失败：{e}", env.peer)))?;
        let proto =
            ProtocolId::new(CHAT_PROTOCOL).map_err(|e| ChatError::Protocol(e.to_string()))?;
        let mut stream = self
            .node
            .new_stream(pid, proto)
            .await
            .map_err(|e| ChatError::StreamFailed(format!("开流失败：{e}")))?;
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
        let ack = match tokio::time::timeout(ACK_TIMEOUT, wire::read_ack(&mut stream)).await {
            Ok(r) => r.map_err(io_err_to_send)?,
            Err(_) => return Err(ChatError::StreamFailed("等待 ACK 超时".into())),
        };
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

    /// failed 条目是否已在本进程内重投过：是则死信出队（消息记录保留 failed）并返回 true。
    pub(crate) fn dead_letter_if_spent(&self, peer: &str, env: &ChatEnvelope) -> bool {
        if env.status != ChatStatus::Failed {
            return false;
        }
        let spent = self
            .flush_tried
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(&(peer.to_string(), env.id.clone()));
        if !spent {
            return false;
        }
        match self.store.remove_outbox(peer, &env.id) {
            Ok(()) => {
                tracing::warn!(peer = %peer, id = %env.id, "outbox 条目重投未愈，死信出队（记录保留 failed）");
                true
            }
            Err(e) => {
                tracing::warn!(peer = %peer, id = %env.id, error = %e, "死信出队失败，保留条目");
                false
            }
        }
    }

    /// 记账：该 failed 条目本进程已重投过一次。
    pub(crate) fn mark_attempted(&self, peer: &str, id: &str) {
        self.flush_tried
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert((peer.to_string(), id.to_string()), ());
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

fn io_err_to_send(e: std::io::Error) -> ChatError {
    ChatError::StreamFailed(format!("流 IO 失败：{e}"))
}
