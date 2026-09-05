//! 群聊核心：/im/group/1 帧事务、goutbox 入队（design §3/§5/§6）。
//! 收敛应用在 group_model.rs，投递泵与记账在 outbox.rs；per-peer 串行锁复用 1:1
//! ChatCore（群流量与 1:1 同连接互斥，规避 yamux 并发开流缺陷）。

use std::sync::Arc;

use p2p::ProtocolId;
use p2p_identity::PeerId;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;

use crate::core::ChatCore;
use crate::group::{GroupRoster, GroupStateAck};
use crate::group_model::{GroupEvent, EVENT_CAPACITY};
use crate::group_store::{GoutboxEntry, GoutboxFrame, GroupInfo, GroupMessage, GroupStore};
use crate::group_wire::{
    read_typed, WireGroupEnvelope, GROUP_PROTOCOL, G_ENVELOPE, G_STATE, G_STATE_ACK,
};
use crate::model::{parse_peer_id, sanitize_name, ChatError, ChatMediaMeta, ChatStatus};
use crate::wire::{read_ack, write_typed, MediaBegin, CHUNK_LEN, MEDIA_BEGIN, MEDIA_CHUNK};

/// 单次投递内 ACK/STATE_ACK 等待上限（同 1:1：死连接上无界等待是演练 D1 卡死面）。
const ACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// 群核心：成员操作落盘收敛 + 帧事务 + goutbox 入队。
pub(crate) struct GroupCore {
    pub(crate) chat: Arc<ChatCore>,
    pub(crate) store: GroupStore,
    pub(crate) events: broadcast::Sender<GroupEvent>,
}

impl GroupCore {
    pub(crate) fn new(chat: Arc<ChatCore>, store: GroupStore) -> Self {
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        Self {
            chat,
            store,
            events,
        }
    }

    /// 传输类失败后的强拆重拨（同 1:1 deliver 纪律，跨机演练 D1 复现面）。
    pub(crate) async fn redial(&self, to: &str) -> Result<(), ChatError> {
        let pid = parse_peer_id(to)?;
        self.chat.node.disconnect(&pid);
        self.chat
            .node
            .connect(pid)
            .await
            .map_err(|e| ChatError::ConnectFailed(format!("连接 {to} 失败：{e}")))
    }

    fn io_send(e: std::io::Error) -> ChatError {
        ChatError::StreamFailed(format!("流 IO 失败：{e}"))
    }

    /// 帧事务公共段：幂等连接 → 开 /im/group/1 流。
    pub(crate) async fn connect_stream(
        &self,
        to: &str,
    ) -> Result<(PeerId, p2p_mux::BoxedStream), ChatError> {
        let pid = parse_peer_id(to)?;
        self.chat
            .node
            .connect(pid)
            .await
            .map_err(|e| ChatError::ConnectFailed(format!("连接 {to} 失败：{e}")))?;
        let proto =
            ProtocolId::new(GROUP_PROTOCOL).map_err(|e| ChatError::Protocol(e.to_string()))?;
        let stream = self
            .chat
            .node
            .new_stream(pid, proto)
            .await
            .map_err(|e| ChatError::StreamFailed(format!("开流失败：{e}")))?;
        Ok((pid, stream))
    }

    /// 消息事务：信封 →（MEDIA_BEGIN→MEDIA_CHUNK×n）→ ACK。
    /// Ok(true) = 已确认；Ok(false) = 对端 unknown_group（发端条目保持 pending 等 roster）。
    pub(crate) async fn send_msg(&self, to: &str, msg: &GroupMessage) -> Result<bool, ChatError> {
        let (_pid, mut stream) = self.connect_stream(to).await?;
        let bytes =
            serde_json::to_vec(&WireGroupEnvelope::from_local(msg)).map_err(ChatError::Json)?;
        write_typed(&mut stream, G_ENVELOPE, &bytes)
            .await
            .map_err(Self::io_send)?;
        if let Some(media) = &msg.media {
            let path = media
                .path
                .as_ref()
                .ok_or_else(|| ChatError::SendFailed("附件路径缺失".into()))?;
            let data = tokio::fs::read(path)
                .await
                .map_err(|e| ChatError::SendFailed(format!("读附件失败：{e}")))?;
            let head = MediaBegin {
                len: data.len() as u64,
                name: media.name.clone(),
                mime: media.mime.clone(),
                kind: msg.kind.clone(),
            };
            let hb = serde_json::to_vec(&head).map_err(ChatError::Json)?;
            write_typed(&mut stream, MEDIA_BEGIN, &hb)
                .await
                .map_err(Self::io_send)?;
            for chunk in data.chunks(CHUNK_LEN) {
                write_typed(&mut stream, MEDIA_CHUNK, chunk)
                    .await
                    .map_err(Self::io_send)?;
            }
        }
        stream.flush().await.map_err(Self::io_send)?;
        let ack = tokio::time::timeout(ACK_TIMEOUT, read_ack(&mut stream))
            .await
            .map_err(|_| ChatError::StreamFailed("等待 ACK 超时".into()))?
            .map_err(Self::io_send)?;
        if ack.id != msg.id {
            return Err(ChatError::SendFailed(format!(
                "ACK id 不匹配：{} ≠ {}",
                ack.id, msg.id
            )));
        }
        if ack.ok {
            return Ok(true);
        }
        if ack.reason.as_deref() == Some("unknown_group") {
            return Ok(false);
        }
        Err(ChatError::SendFailed(format!(
            "对端拒绝：{}",
            ack.reason.as_deref().unwrap_or("")
        )))
    }

    /// 入站收媒体（design §6.4）：MEDIA_BEGIN 校验 → CHUNK×n 落 tmp → rename media/<groupId>/。
    pub(crate) async fn receive_media(
        &self,
        stream: &mut p2p_mux::BoxedStream,
        group_id: &str,
        msg_id: &str,
        meta: &ChatMediaMeta,
    ) -> std::io::Result<std::path::PathBuf> {
        let head: MediaBegin =
            serde_json::from_slice(&read_typed(stream, MEDIA_BEGIN, "MEDIA_BEGIN").await?)
                .map_err(|e| std::io::Error::other(format!("媒体头 JSON 非法：{e}")))?;
        if head.len != meta.size {
            return Err(std::io::Error::other(format!(
                "媒体长度不一致：头 {} ≠ 信封 {}",
                head.len, meta.size
            )));
        }
        let dir = self.store.media_group_dir(group_id)?;
        let final_path = dir.join(format!("{msg_id}_{}", sanitize_name(&meta.name)));
        let tmp = dir.join(format!(".tmp-{msg_id}-{}", std::process::id()));
        let mut file = tokio::fs::File::create(&tmp).await?;
        let mut received: u64 = 0;
        while received < head.len {
            let payload = read_typed(stream, MEDIA_CHUNK, "MEDIA_CHUNK").await?;
            received += payload.len() as u64;
            if received > head.len {
                let _ = tokio::fs::remove_file(&tmp).await;
                return Err(std::io::Error::other("媒体超过声明长度，断流"));
            }
            file.write_all(&payload).await?;
        }
        file.flush().await?;
        drop(file);
        tokio::fs::rename(&tmp, &final_path).await?;
        Ok(final_path)
    }

    /// roster 事务：G_STATE → G_STATE_ACK；ok=false 或回执不匹配即 Err（拒收可观测）。
    pub(crate) async fn send_roster(
        &self,
        to: &str,
        roster: &GroupRoster,
    ) -> Result<(), ChatError> {
        let (_pid, mut stream) = self.connect_stream(to).await?;
        let bytes = serde_json::to_vec(roster).map_err(ChatError::Json)?;
        write_typed(&mut stream, G_STATE, &bytes)
            .await
            .map_err(Self::io_send)?;
        stream.flush().await.map_err(Self::io_send)?;
        let payload = tokio::time::timeout(
            ACK_TIMEOUT,
            read_typed(&mut stream, G_STATE_ACK, "G_STATE_ACK"),
        )
        .await
        .map_err(|_| ChatError::StreamFailed("等待 G_STATE_ACK 超时".into()))?
        .map_err(Self::io_send)?;
        let ack: GroupStateAck = serde_json::from_slice(&payload)
            .map_err(|e| ChatError::StreamFailed(format!("G_STATE_ACK JSON 非法：{e}")))?;
        if ack.group_id != roster.group_id || ack.rev != roster.rev {
            return Err(ChatError::SendFailed(format!(
                "G_STATE_ACK 不匹配：{}/{}",
                ack.group_id, ack.rev
            )));
        }
        if !ack.ok {
            return Err(ChatError::SendFailed(format!(
                "对端拒收 roster：{}",
                ack.reason.as_deref().unwrap_or("")
            )));
        }
        Ok(())
    }

    /// G_KICK/G_LEAVE 单向通知（best-effort）：写帧 flush 即成（无回执）。
    pub(crate) async fn send_notice<T: serde::Serialize>(
        &self,
        to: &str,
        kind: u8,
        payload: &T,
    ) -> Result<(), ChatError> {
        let (_pid, mut stream) = self.connect_stream(to).await?;
        let bytes = serde_json::to_vec(payload).map_err(ChatError::Json)?;
        write_typed(&mut stream, kind, &bytes)
            .await
            .map_err(Self::io_send)?;
        stream.flush().await.map_err(Self::io_send)
    }

    /// 先落 goutbox（异常原子性）→ 命令内补投积压 → 投递本条；失败留条目等 flush。
    pub(crate) async fn push_frame(&self, to: &str, frame: GoutboxFrame) -> Result<(), ChatError> {
        let entry = GoutboxEntry {
            id: uuid::Uuid::new_v4().to_string(),
            to: to.to_string(),
            status: ChatStatus::Pending,
            attempts: 0,
            frame,
        };
        self.store.append_goutbox(&entry).map_err(ChatError::Io)?;
        let _guard = self.chat.peer_guard(to).await;
        // 命令内补投（design §6.2 一次性命令加法）：先补该成员既有积压（不含本条），
        // 退出前投完或显式失败留痕，禁后台任务与进程退出的竞态；与后台 flush 共享
        // 条目落盘 attempts 台账。常驻 serve 的 PeerConnected flush 语义不变。
        if let Err(e) = crate::outbox::flush_entries(self, to, Some(&entry.id)).await {
            tracing::warn!(to = %to, error = %e, "命令内 goutbox 补投失败，留待后续连接");
        }
        match crate::outbox::attempt(self, &entry).await {
            Ok(_) => {}
            // 对端离线/不可达：条目已入队且不计数，PeerConnected 后 flush（design §6.2）
            Err(ChatError::ConnectFailed(_)) => return Ok(()),
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// 向全体其他成员推 roster（owner 权威广播；离线成员 goutbox 补投，§5 最终一致）。
    pub(crate) async fn push_roster_all(&self, group: &GroupInfo) -> Result<(), ChatError> {
        let roster = GroupRoster::from_group(group);
        for member in self.others(group) {
            self.push_frame(
                &member,
                GoutboxFrame::Roster {
                    roster: Box::new(roster.clone()),
                },
            )
            .await?;
        }
        Ok(())
    }
}
