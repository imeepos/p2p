//! 群聊公共模型（design im-group-design.md §7）与 GroupCore 收敛应用/公共助手。
//! 序列化形状逐字对齐契约（tag=type，camelCase 字段），G4/G5 按此消费；
//! impl 分文件同居 group.rs / group_model.rs / outbox.rs（300 行红线再平衡）。

use serde::{Deserialize, Serialize};

use crate::group::{GroupKick, GroupLeave, GroupRoster};
use crate::group_store::{GroupInfo, GroupMessage, GroupState};
use crate::model::{now_ms, parse_peer_id, ChatError, ChatMediaMeta, ChatStatus};

/// 群事件通道容量（独立于 1:1 事件通道）。
pub(crate) const EVENT_CAPACITY: usize = 128;

/// 群命令统一返回别名（GUI 侧可读中文 Err）。
pub(crate) type GroupResult<T> = Result<T, ChatError>;

/// 群事件（契约 §7 判别联合；chat_group_message / chat_group_status / chat_group_state）。
#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GroupEvent {
    #[serde(rename = "chat_group_message")]
    Message {
        group_id: String,
        message: GroupMessage,
    },
    #[serde(rename = "chat_group_status")]
    Status {
        group_id: String,
        message_id: String,
        acks: Vec<String>,
        status: ChatStatus,
    },
    #[serde(rename = "chat_group_state")]
    State { group: GroupInfo },
}

/// group_send 返回（契约 §7）：acked = 本轮确认数；delivered = acked == recipients。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupSendReport {
    pub message: GroupMessage,
    pub acked: usize,
    pub recipients: usize,
    pub delivered: bool,
}

impl crate::group_core::GroupCore {
    pub(crate) fn emit(&self, ev: GroupEvent) {
        let _ = self.events.send(ev);
    }

    /// 变更公共尾：落盘（失败留告警）+ chat_group_state 事件。
    pub(crate) fn commit(&self, group: GroupInfo) -> Result<(), ChatError> {
        if let Err(e) = self.store.save_group(group.clone()) {
            tracing::warn!(group_id = %group.group_id, error = %e, "群状态落盘失败");
            return Err(e.into());
        }
        self.emit(GroupEvent::State { group });
        Ok(())
    }

    /// 群成员中除本机外的目标集（fan-out / roster 推送对象）。
    pub(crate) fn others(&self, group: &GroupInfo) -> Vec<String> {
        let local = self.chat.node.local_peer_id().to_string();
        group
            .members
            .iter()
            .filter(|m| **m != local)
            .cloned()
            .collect()
    }

    /// 群在册校验（不存在即 Err 可读中文）。
    pub(crate) fn require_group(&self, group_id: &str) -> Result<GroupInfo, ChatError> {
        self.store
            .group(group_id)
            .ok_or_else(|| ChatError::NotFound(format!("群不存在：{group_id}")))
    }

    /// 入站 roster 收敛（design §3.2）：首见落定 owner；owner 绑定不符拒收；
    /// rev ≤ 本地幂等丢弃；高 rev 胜且 state 回 active（被移出后重邀可回归）。
    pub(crate) fn apply_roster(&self, roster: &GroupRoster) -> Result<(), String> {
        let local = self.chat.node.local_peer_id().to_string();
        if let Some(group) = self.store.group(&roster.group_id) {
            if group.owner == local {
                return Err("本机是该群 owner，拒收外来 roster".into());
            }
            if roster.owner != group.owner {
                return Err(format!(
                    "owner 绑定不符：本地 {} ≠ roster {}",
                    group.owner, roster.owner
                ));
            }
            if roster.rev <= group.rev {
                return Ok(());
            }
            let mut next = group;
            next.name = roster.name.clone();
            next.members = roster.members.clone();
            next.rev = roster.rev;
            next.state = GroupState::Active;
            next.ts_ms = roster.ts_ms;
            return self.commit(next).map_err(|e| e.to_string());
        }
        self.commit(GroupInfo::from_roster(roster))
            .map_err(|e| e.to_string())
    }

    /// G_KICK 应用（design §5，幂等）：state 置位，历史保留；未知群告警忽略。
    pub(crate) fn apply_kick(&self, kick: &GroupKick) -> Result<(), ChatError> {
        if kick.reason != "kicked" && kick.reason != "disbanded" {
            return Err(ChatError::Protocol(format!(
                "G_KICK reason 非法：{}",
                kick.reason
            )));
        }
        let Some(mut group) = self.store.group(&kick.group_id) else {
            tracing::warn!(group_id = %kick.group_id, "G_KICK 群不存在，忽略");
            return Ok(());
        };
        if group.owner == self.chat.node.local_peer_id().to_string() {
            tracing::warn!(group_id = %kick.group_id, "本机是该群 owner，拒收外来 G_KICK");
            return Ok(());
        }
        let state = if kick.reason == "kicked" {
            GroupState::Kicked
        } else {
            GroupState::Disbanded
        };
        if group.state != state {
            group.state = state;
            self.commit(group.clone())?;
        }
        Ok(())
    }

    /// G_LEAVE 应用（owner 端，design §5）：rev+1 移除退群者并推余员 roster；
    /// 非 owner 收到或退群者不在群 → 告警忽略（幂等，最终一致）。
    pub(crate) async fn apply_leave(&self, leave: &GroupLeave) -> Result<(), ChatError> {
        let local = self.chat.node.local_peer_id().to_string();
        let mut group = self
            .store
            .group(&leave.group_id)
            .ok_or_else(|| ChatError::NotFound(format!("群不存在：{}", leave.group_id)))?;
        if group.owner != local {
            tracing::warn!(group_id = %leave.group_id, sender = %leave.sender, "非 owner 收到 G_LEAVE，忽略");
            return Ok(());
        }
        if !group.members.contains(&leave.sender) {
            tracing::warn!(group_id = %leave.group_id, sender = %leave.sender, "G_LEAVE 成员不在群，忽略");
            return Ok(());
        }
        group.members.retain(|m| *m != leave.sender);
        group.rev += 1;
        group.ts_ms = now_ms();
        self.commit(group.clone())?;
        self.push_roster_all(&group).await
    }
}

impl crate::group::Group {
    /// 全量群列表（含 left/kicked/disbanded，GUI 按 state 过滤/置底）。
    pub fn group_list(&self) -> Vec<GroupInfo> {
        self.core.store.groups_list()
    }

    /// 群历史分页：time desc；beforeId 游标；limit 默认 50 上限 100（同 1:1）。
    pub fn group_history(
        &self,
        group_id: &str,
        before_id: Option<&str>,
        limit: usize,
    ) -> GroupResult<Vec<GroupMessage>> {
        let mut msgs = self.core.store.history(group_id);
        if let Some(before) = before_id {
            let ts = msgs
                .iter()
                .find(|m| m.id == before)
                .ok_or_else(|| ChatError::NotFound(format!("游标消息不存在：{before}")))?
                .ts_ms;
            msgs.retain(|m| m.ts_ms < ts);
        }
        msgs.sort_by_key(|m| std::cmp::Reverse(m.ts_ms));
        msgs.truncate(if limit == 0 { 50 } else { limit.min(100) });
        Ok(msgs)
    }

    /// 群附件本端绝对路径（非媒体/不存在 → Err），目录 = media/<groupId>/。
    pub fn group_media_file(&self, group_id: &str, message_id: &str) -> GroupResult<ChatMediaMeta> {
        self.core
            .store
            .history(group_id)
            .into_iter()
            .find(|m| m.id == message_id)
            .and_then(|m| m.media)
            .ok_or_else(|| ChatError::NotFound("消息不存在或非附件类型".into()))
    }

    /// owner 名单/信息变更公共尾：rev+1 → 落盘事件 → 推全体 roster。
    pub(crate) async fn bump_push(&self, group: &mut GroupInfo) -> GroupResult<()> {
        group.rev += 1;
        group.ts_ms = now_ms();
        self.core.commit(group.clone())?;
        self.core.push_roster_all(group).await
    }

    /// 成员入参校验（建群/邀请共用）：合法、非本机、∈ 好友簿；dedup=true 幂等跳重。
    pub(crate) fn check_members(
        &self,
        ids: &[String],
        members: &[String],
        dedup: bool,
    ) -> GroupResult<Vec<String>> {
        let mut added = Vec::new();
        let local = self.core.chat.node.local_peer_id().to_string();
        let friends = self.core.chat.store.friends_list()?;
        for id in ids {
            if parse_peer_id(id)?.to_string() == local {
                return Err(ChatError::SelfPeer(id.clone()));
            }
            if members.contains(id) || added.contains(id) {
                if dedup {
                    continue;
                }
                return Err(ChatError::InvalidUpdate(format!("已在群中：{id}")));
            }
            if !friends.iter().any(|f| &f.peer_id == id) {
                return Err(ChatError::NotFound(format!("成员不在好友簿：{id}")));
            }
            added.push(id.clone());
        }
        Ok(added)
    }

    /// 群存在 + 本机是 owner（owner 操作前置校验）。
    pub(crate) fn owned_group(&self, group_id: &str) -> GroupResult<GroupInfo> {
        let group = self.core.require_group(group_id)?;
        if group.owner != self.core.chat.node.local_peer_id().to_string() {
            return Err(ChatError::InvalidUpdate("仅群主可执行该操作".into()));
        }
        Ok(group)
    }

    /// 发送回执：全员确认时历史落 delivered；acks 取磁盘真值。
    pub(crate) fn finish_report(
        &self,
        group_id: &str,
        mut msg: GroupMessage,
        recipients: usize,
        acked: usize,
    ) -> GroupSendReport {
        let delivered = acked >= recipients;
        if delivered {
            let saved = self
                .core
                .store
                .patch_message(group_id, &msg.id, |m| m.status = ChatStatus::Delivered);
            if let Err(e) = saved {
                tracing::warn!(group_id = %group_id, id = %msg.id, error = %e, "delivered 状态落盘失败");
            }
        }
        msg.status = if delivered {
            ChatStatus::Delivered
        } else {
            ChatStatus::Pending
        };
        if let Some(stored) = self
            .core
            .store
            .history(group_id)
            .into_iter()
            .find(|m| m.id == msg.id)
        {
            msg.acks = stored.acks;
        }
        GroupSendReport {
            message: msg,
            acked,
            recipients,
            delivered,
        }
    }
}
