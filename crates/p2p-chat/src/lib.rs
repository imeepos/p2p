//! p2p-chat：IM 聊天核心 crate（design §1-§6；契约 gui-contract.md §12）。
//!
//! 好友簿 / 消息历史 / outbox 离线队列 / 线协议 /im/chat/1 收发；底座 p2p-* 只读。
//! 发送：校验 → 落 outbox → 连接 → 开流帧序 → ACK → delivered；
//! 入站：handler 读信封 → 收媒体 → 回 ACK → 落盘 → chat_message 事件。

mod core;
mod friend;
mod identity_lock;
mod model;
mod outbox;
mod store;
mod store_io;
mod store_lock;
mod wire;

pub use identity_lock::try_lock_identity;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use p2p::Node;
use tokio::sync::broadcast;

pub use friend::{validate_group, ChatFriend, FriendPatch, MAX_GROUP_CHARS};
pub use model::{
    sanitize_name, validate_media, validate_text, ChatEnvelope, ChatError, ChatEvent, ChatKind,
    ChatMediaInput, ChatMediaMeta, ChatSendReport, ChatStatus, Sender, MAX_MESSAGE_SIZE,
};

use core::ChatCore;

/// 线协议 ID（wire-protocol.md §8 登记）。
pub const CHAT_PROTOCOL: &str = "/im/chat/1";

/// 事件通道容量。
const EVENT_CAPACITY: usize = 128;

/// 聊天门面：好友簿 / 发送 / 历史 / 媒体路径，内部持有核心与 outbox 任务。
pub struct Chat {
    core: Arc<ChatCore>,
}

impl Chat {
    /// 装配：建存储、注册入站 handler、启动 outbox 任务、好友地址回填地址簿。
    pub fn new(node: Arc<Node>, data_dir: PathBuf) -> Result<Self, ChatError> {
        let store = store::Store::new(data_dir.join("chat"))?;
        let (tx, _) = broadcast::channel(EVENT_CAPACITY);
        let core = Arc::new(ChatCore {
            node,
            store,
            events: tx.clone(),
            send_locks: std::sync::Mutex::new(std::collections::HashMap::new()),
            flush_tried: std::sync::Mutex::new(std::collections::HashMap::new()),
        });
        core.rearm_friend_addrs()?;
        let proto =
            p2p::ProtocolId::new(CHAT_PROTOCOL).map_err(|e| ChatError::Protocol(e.to_string()))?;
        core.node
            .handle_protocol(Arc::new(wire::ChatHandler::new(core.clone(), proto)));
        outbox::spawn_outbox_task(core.clone());
        Ok(Self { core })
    }

    /// chat_message / chat_status 事件订阅。
    pub fn events(&self) -> broadcast::Receiver<ChatEvent> {
        self.core.events.subscribe()
    }

    /// 好友簿列表（无文件返回空数组）。
    pub fn friends_list(&self) -> Result<Vec<ChatFriend>, ChatError> {
        Ok(self.core.store.friends_list()?)
    }

    /// 加好友：校验 peerId（base58 且 ≠ 本机）/昵称（trim ≤64）/地址语法，
    /// 通过后原子写好友簿并同步登记地址簿（可拨）。
    pub fn friend_add(
        &self,
        peer_id: &str,
        nickname: &str,
        addrs: Vec<String>,
        note: Option<String>,
    ) -> Result<ChatFriend, ChatError> {
        let peer = model::parse_peer_id(peer_id)?;
        if peer == self.core.node.local_peer_id() {
            return Err(ChatError::SelfPeer(peer_id.to_string()));
        }
        let nickname = model::validate_nickname(nickname)?;
        for addr in &addrs {
            self.core
                .node
                .add_peer_address(peer, addr)
                .map_err(|e| ChatError::InvalidAddr(format!("{addr}: {e}")))?;
        }
        let friend = ChatFriend {
            peer_id: peer_id.to_string(),
            nickname,
            addrs,
            note,
            group: None,
        };
        self.core.store.upsert_friend(friend.clone())?;
        Ok(friend)
    }

    /// 移除好友（幂等，never 在簿返回 false；不删消息历史）。
    pub fn friend_remove(&self, peer_id: &str) -> Result<bool, ChatError> {
        let _ = model::parse_peer_id(peer_id)?;
        Ok(self.core.store.remove_friend(peer_id)?)
    }

    /// 更新好友资料补丁（IM-T43）：group/nickname/note 至少一项；addrs 不可经此修改；
    /// peer 不在簿 Err。group 提供空串 = 移出分组（归一化 None，落盘无空串组名）。
    pub fn friend_update(
        &self,
        peer_id: &str,
        patch: &FriendPatch,
    ) -> Result<ChatFriend, ChatError> {
        let _ = model::parse_peer_id(peer_id)?;
        if patch.is_empty() {
            return Err(ChatError::InvalidUpdate(
                "更新内容为空：group/nickname/note 至少提供一项".into(),
            ));
        }
        let group = friend::validate_group(patch.group.as_deref())?;
        let nickname = patch
            .nickname
            .as_deref()
            .map(model::validate_nickname)
            .transpose()?;
        let note = match patch.note.as_deref() {
            Some(n) if n.trim().is_empty() => Some(None),
            Some(n) => Some(Some(n.to_string())),
            None => None,
        };
        let mut friends = self.core.store.friends_list()?;
        let slot = friends
            .iter_mut()
            .find(|f| f.peer_id == peer_id)
            .ok_or_else(|| ChatError::NotFound(format!("好友不在簿：{peer_id}")))?;
        if patch.group.is_some() {
            slot.group = group;
        }
        if let Some(name) = nickname {
            slot.nickname = name;
        }
        if let Some(value) = note {
            slot.note = value;
        }
        let updated = slot.clone();
        self.core.store.upsert_friend(updated.clone())?;
        Ok(updated)
    }

    /// 历史分页：time desc；beforeId = 严格更早游标；limit 默认 50 上限 100。
    pub fn history(
        &self,
        peer: &str,
        before_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ChatEnvelope>, ChatError> {
        let _ = model::parse_peer_id(peer)?;
        let mut msgs = self.core.store.messages_for(peer)?;
        if let Some(before) = before_id {
            let before_ts = msgs
                .iter()
                .find(|m| m.id == before)
                .ok_or_else(|| ChatError::NotFound(format!("游标消息不存在：{before}")))?
                .ts_ms;
            msgs.retain(|m| m.ts_ms < before_ts);
        }
        msgs.sort_by_key(|a| std::cmp::Reverse(a.ts_ms));
        let cap = if limit == 0 { 50 } else { limit.min(100) };
        msgs.truncate(cap);
        Ok(msgs)
    }

    /// 附件落盘绝对路径（仅本端展示用）；非媒体消息或不存在 → Err。
    pub fn media_file(&self, peer: &str, message_id: &str) -> Result<ChatMediaMeta, ChatError> {
        let msgs = self.core.store.messages_for(peer)?;
        let msg = msgs
            .iter()
            .find(|m| m.id == message_id)
            .ok_or_else(|| ChatError::NotFound(format!("消息不存在：{message_id}")))?;
        msg.media
            .clone()
            .ok_or_else(|| ChatError::NotFound("消息非附件类型".into()))
    }

    /// 发送：校验 → 信封 → 落 outbox/messages(pending) → 实时投递；
    /// 对端未连接保持 pending，等待 PeerConnected 事件 flush。
    pub async fn send(
        &self,
        peer: &str,
        kind: ChatKind,
        text: Option<String>,
        media: Option<ChatMediaInput>,
        reply_to: Option<String>,
    ) -> Result<ChatSendReport, ChatError> {
        let peer_id = model::parse_peer_id(peer)?;
        if peer_id == self.core.node.local_peer_id() {
            return Err(ChatError::SelfPeer(peer.to_string()));
        }
        // 回复引用校验：提供时必须非空字符串；不校验被引用消息存在性（离线引用允许）。
        let reply_to = match reply_to.as_deref() {
            None => None,
            Some(s) if s.trim().is_empty() => {
                return Err(ChatError::InvalidReply(s.to_string()));
            }
            Some(s) => Some(s.to_string()),
        };
        let text = if kind == ChatKind::Text {
            Some(model::validate_text(text.as_deref().unwrap_or_default())?)
        } else {
            None
        };
        let mut env = ChatEnvelope {
            id: uuid::Uuid::new_v4().to_string(),
            peer: peer.to_string(),
            sender: Sender::Me,
            kind: kind.clone(),
            ts_ms: model::now_ms(),
            text,
            media: None,
            status: ChatStatus::Pending,
            reply_to,
        };
        match (&kind, media) {
            (ChatKind::Text, Some(_)) => {
                return Err(ChatError::InvalidMedia("text 消息不能携带附件".into()));
            }
            (ChatKind::Text, None) => {}
            (other, None) => {
                return Err(ChatError::InvalidMedia(format!("{other} 消息必须携带附件")));
            }
            (_, Some(input)) => {
                model::validate_media(&env.kind, &input.mime, input.data.len() as u64)?;
                let path = self
                    .core
                    .store
                    .save_media(peer, &env.id, &input.name, &input.data)?;
                env.media = Some(ChatMediaMeta {
                    name: input.name,
                    mime: input.mime,
                    size: input.data.len() as u64,
                    path: Some(path.to_string_lossy().into_owned()),
                });
            }
        }
        self.core.store.append_outbox(&env)?;
        self.core.store.append_message(&env)?;
        self.core.emit_status(peer, &env.id, ChatStatus::Pending);
        // 持有 peer 投递锁：connect 触发的 PeerConnected 会让 outbox 任务并发 flush
        // 同一条消息，串行化避免同连接并发开流（yamux 上游缺陷，见 core.rs 注释）。
        let _guard = self.core.peer_guard(peer).await;
        let delivered = match self.core.deliver(&env).await {
            Ok(()) => {
                self.core.mark_delivered(peer, &env)?;
                true
            }
            Err(ChatError::ConnectFailed(e)) => {
                tracing::warn!(peer = %peer, id = %env.id, error = %e, "对端未连接，消息保持 pending");
                false
            }
            Err(e) => {
                tracing::warn!(peer = %peer, id = %env.id, error = %e, "发送失败，消息标记 failed");
                self.core.mark_failed(peer, &env)?;
                false
            }
        };
        let status = self.core.status_of(peer, &env.id)?;
        env.status = status;
        Ok(ChatSendReport {
            message: env,
            delivered,
        })
    }

    /// 排空该 peer 的 outbox（CLI one-shot 收尾用，D5）：主动连接后 flush，budget 内尽力而为。
    /// 返回本轮成功补投的条目数（按队列长度差计）。
    pub async fn drain_peer(&self, peer: &str, budget: Duration) -> Result<usize, ChatError> {
        let pid = model::parse_peer_id(peer)?;
        let before = self.core.store.outbox_for(peer).len();
        if before == 0 {
            return Ok(0);
        }
        self.core
            .node
            .connect(pid)
            .await
            .map_err(|e| ChatError::ConnectFailed(format!("连接 {peer} 失败：{e}")))?;
        let _ = tokio::time::timeout(budget, outbox::flush_peer(&self.core, peer)).await;
        let after = self.core.store.outbox_for(peer).len();
        Ok(before.saturating_sub(after))
    }
}
