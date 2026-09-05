//! 群消息发送路径（design im-group-design.md §6.1/§6.2）：校验 → 信封 → 历史 pending
//! → 每目标 goutbox → 串行 fan-out。自 group.rs / outbox.rs 原样迁入（300 行红线再平衡）。

use crate::group::Group;
use crate::group_model::{GroupResult, GroupSendReport};
use crate::group_store::{GoutboxEntry, GoutboxFrame, GroupInfo, GroupMessage, GroupState};
use crate::model::{
    now_ms, validate_media, validate_text, ChatError, ChatKind, ChatMediaInput, ChatMediaMeta,
    ChatStatus,
};
use crate::outbox;

impl Group {
    /// 发送前置校验：群在册、state=active、本机在成员名单（被踢/退群/解散禁发）。
    pub(crate) fn sendable_group(&self, group_id: &str) -> Result<GroupInfo, ChatError> {
        let group = self.core.require_group(group_id)?;
        let local = self.core.chat.node.local_peer_id().to_string();
        if group.state != GroupState::Active {
            return Err(ChatError::InvalidUpdate(
                "群已退出/被移出/已解散，禁止发送".into(),
            ));
        }
        if !group.members.contains(&local) {
            return Err(ChatError::InvalidUpdate(
                "本机不在群成员名单，禁止发送".into(),
            ));
        }
        Ok(group)
    }

    /// 信封构建与校验（同 1:1 白名单/上限；text/附件互斥；回复引用非空即可，不验存在性）。
    pub(crate) fn build_message(
        &self,
        group: &GroupInfo,
        kind: ChatKind,
        text: Option<String>,
        media: Option<ChatMediaInput>,
        reply_to: Option<String>,
    ) -> Result<GroupMessage, ChatError> {
        if let Some(r) = reply_to.as_deref() {
            if r.trim().is_empty() {
                return Err(ChatError::InvalidReply(r.to_string()));
            }
        }
        let text = if kind == ChatKind::Text {
            Some(validate_text(text.as_deref().unwrap_or_default())?)
        } else {
            None
        };
        let mut msg = GroupMessage {
            id: uuid::Uuid::new_v4().to_string(),
            group_id: group.group_id.clone(),
            sender_id: self.core.chat.node.local_peer_id().to_string(),
            kind: kind.clone(),
            ts_ms: now_ms(),
            text,
            media: None,
            status: ChatStatus::Pending,
            acks: Vec::new(),
            reply_to,
        };
        match (&kind, media) {
            (ChatKind::Text, Some(_)) => {
                return Err(ChatError::InvalidMedia("text 消息不能携带附件".into()));
            }
            (ChatKind::Text, None) => {}
            (kind, None) => {
                return Err(ChatError::InvalidMedia(format!("{kind} 消息必须携带附件")))
            }
            (kind, Some(input)) => {
                validate_media(kind, &input.mime, input.data.len() as u64)?;
                let path = self
                    .core
                    .store
                    .save_media(&group.group_id, &msg.id, &input.name, &input.data)
                    .map_err(ChatError::Io)?;
                msg.media = Some(ChatMediaMeta {
                    name: input.name,
                    mime: input.mime,
                    size: input.data.len() as u64,
                    path: Some(path.to_string_lossy().into_owned()),
                });
            }
        }
        Ok(msg)
    }

    /// 群消息发送（design §6.1）：历史落 pending → 每目标写 goutbox → 串行 fan-out。
    pub async fn group_send(
        &self,
        group_id: &str,
        kind: ChatKind,
        text: Option<String>,
        media: Option<ChatMediaInput>,
        reply_to: Option<String>,
    ) -> GroupResult<GroupSendReport> {
        let group = self.sendable_group(group_id)?;
        let msg = self.build_message(&group, kind, text, media, reply_to)?;
        let targets = self.core.others(&group);
        self.core
            .store
            .append_message(&msg)
            .map_err(ChatError::Io)?;
        let mut entries = Vec::with_capacity(targets.len());
        for to in &targets {
            let entry = GoutboxEntry {
                id: uuid::Uuid::new_v4().to_string(),
                to: to.clone(),
                status: ChatStatus::Pending,
                frame: GoutboxFrame::Msg {
                    msg: Box::new(msg.clone()),
                },
            };
            self.core
                .store
                .append_goutbox(&entry)
                .map_err(ChatError::Io)?;
            entries.push(entry);
        }
        let mut acked = 0usize;
        for entry in &entries {
            let _guard = self.core.chat.peer_guard(&entry.to).await;
            if outbox::attempt(&self.core, entry).await.unwrap_or(false) {
                acked += 1;
            }
        }
        Ok(self.finish_report(&group.group_id, msg, targets.len(), acked))
    }
}
