//! 群聊门面（design im-group-design.md §7 契约加法）：owner 权威模型，非 owner 的
//! owner 操作显式 Err，参数无效一律可读中文 Err；1:1 API 与 /im/chat/1 零改动。
//! 模型在 group_model.rs，发送路径校验/信封构建同居 outbox.rs（行数红线再平衡）。

use std::path::Path;
use std::sync::Arc;

use p2p::ProtocolId;
use tokio::sync::broadcast;

use crate::core::ChatCore;
use crate::group_model::GroupResult;
use crate::group_store::{
    validate_group_name, GoutboxEntry, GoutboxFrame, GroupStore, MAX_GROUP_MEMBERS,
};
use crate::group_wire::GroupHandler;
pub use crate::group_wire::GROUP_PROTOCOL;
use crate::model::{now_ms, parse_peer_id, ChatError, ChatKind, ChatMediaInput, ChatStatus};
use crate::outbox;

pub use crate::group_model::{GroupEvent, GroupSendReport};
pub use crate::group_store::{GroupInfo, GroupMessage, GroupState};

/// 群聊门面：建群/成员操作/发送/历史/媒体路径（挂载于 Chat::new）。
pub struct Group {
    pub(crate) core: Arc<crate::group_core::GroupCore>,
}

impl Group {
    /// 装配：建群存储、注册 /im/group/1 入站 handler（goutbox 泵由 outbox.rs 统一驱动）。
    pub(crate) fn mount(chat: Arc<ChatCore>, data_dir: &Path) -> Result<Self, ChatError> {
        let store = GroupStore::new(data_dir.join("chat"))?;
        let core = Arc::new(crate::group_core::GroupCore::new(chat, store));
        let proto =
            ProtocolId::new(GROUP_PROTOCOL).map_err(|e| ChatError::Protocol(e.to_string()))?;
        core.chat
            .node
            .handle_protocol(Arc::new(GroupHandler::new(core.clone(), proto)));
        Ok(Self { core })
    }

    /// chat_group_message / chat_group_status / chat_group_state 事件订阅。
    pub fn events(&self) -> broadcast::Receiver<GroupEvent> {
        self.core.events.subscribe()
    }

    /// 建群：成员 ⊆ 好友簿、≤32、不含本机、群名 trim 1..=64；rev=0 后推 roster。
    pub async fn group_create(&self, name: &str, member_ids: &[String]) -> GroupResult<GroupInfo> {
        let name = validate_group_name(name)?;
        let local = self.core.chat.node.local_peer_id().to_string();
        let mut members = vec![local];
        members.extend(self.check_members(member_ids, &members, true)?);
        if members.len() > MAX_GROUP_MEMBERS {
            return Err(ChatError::InvalidUpdate(format!(
                "群成员超过 {MAX_GROUP_MEMBERS} 人上限"
            )));
        }
        let group = GroupInfo {
            group_id: uuid::Uuid::new_v4().to_string(),
            name,
            owner: members[0].clone(),
            members,
            rev: 0,
            state: GroupState::Active,
            ts_ms: now_ms(),
        };
        self.core.commit(group.clone())?;
        self.core.push_roster_all(&group).await?;
        Ok(group)
    }

    /// 邀请（owner-only）：受邀者 ∈ 好友簿、不在群、群 <32 → rev+1 推全体（含新成员）。
    pub async fn group_invite(
        &self,
        group_id: &str,
        member_ids: &[String],
    ) -> GroupResult<GroupInfo> {
        let mut group = self.owned_group(group_id)?;
        let added = self.check_members(member_ids, &group.members, false)?;
        if group.members.len() + added.len() > MAX_GROUP_MEMBERS {
            return Err(ChatError::InvalidUpdate(format!(
                "群成员将超过 {MAX_GROUP_MEMBERS} 人上限"
            )));
        }
        group.members.extend(added);
        self.bump_push(&mut group).await?;
        Ok(group)
    }

    /// 移除成员（owner-only）：rev+1 推余员，对被移者发 G_KICK(reason=kicked)。
    pub async fn group_kick(&self, group_id: &str, member_id: &str) -> GroupResult<GroupInfo> {
        let mut group = self.owned_group(group_id)?;
        if member_id == group.owner {
            return Err(ChatError::InvalidUpdate("不能移除群主".into()));
        }
        if !group.members.contains(&member_id.to_string()) {
            return Err(ChatError::NotFound(format!("成员不在群中：{member_id}")));
        }
        group.members.retain(|m| *m != member_id);
        let kick = GroupKick {
            group_id: group.group_id.clone(),
            rev: group.rev + 1,
            reason: "kicked".into(),
        };
        self.bump_push(&mut group).await?;
        self.core
            .push_frame(
                member_id,
                GoutboxFrame::Kick {
                    kick: Box::new(kick),
                },
            )
            .await?;
        Ok(group)
    }

    /// 退群：本端 state=left（历史保留）；向 owner 发 G_LEAVE（离线补投）。
    pub async fn group_leave(&self, group_id: &str) -> GroupResult<GroupInfo> {
        let mut group = self.core.require_group(group_id)?;
        let local = self.core.chat.node.local_peer_id().to_string();
        if group.owner == local {
            return Err(ChatError::InvalidUpdate("群主不能退群，请改用解散".into()));
        }
        if group.state != GroupState::Active {
            return Err(ChatError::InvalidUpdate(format!(
                "群状态 {:?}，不可退群",
                group.state
            )));
        }
        group.state = GroupState::Left;
        group.ts_ms = now_ms();
        self.core.commit(group.clone())?;
        let leave = GroupLeave {
            group_id: group.group_id.clone(),
            sender: local,
        };
        self.core
            .push_frame(
                &group.owner,
                GoutboxFrame::Leave {
                    leave: Box::new(leave),
                },
            )
            .await?;
        Ok(group)
    }

    /// 解散（owner-only）：rev+1，对全体成员发 G_KICK(reason=disbanded)，本端 state 置位。
    pub async fn group_disband(&self, group_id: &str) -> GroupResult<GroupInfo> {
        let mut group = self.owned_group(group_id)?;
        group.state = GroupState::Disbanded;
        let mut queued = Vec::new();
        for member in self.core.others(&group) {
            let kick = GroupKick {
                group_id: group.group_id.clone(),
                rev: group.rev + 1,
                reason: "disbanded".into(),
            };
            queued.push((
                member,
                GoutboxFrame::Kick {
                    kick: Box::new(kick),
                },
            ));
        }
        self.bump_push(&mut group).await?;
        for (member, frame) in queued {
            self.core.push_frame(&member, frame).await?;
        }
        Ok(group)
    }

    /// 改名（owner-only）：校验群名 → rev+1 推 roster。
    pub async fn group_rename(&self, group_id: &str, name: &str) -> GroupResult<GroupInfo> {
        let mut group = self.owned_group(group_id)?;
        group.name = validate_group_name(name)?;
        self.bump_push(&mut group).await?;
        Ok(group)
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
/// roster（design §3.2）：members 全量含 owner；rev 单调递增且仅 owner。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupRoster {
    pub(crate) group_id: String,
    pub(crate) name: String,
    pub(crate) owner: String,
    pub(crate) members: Vec<String>,
    pub(crate) rev: u64,
    pub(crate) ts_ms: i64,
}

impl GroupRoster {
    pub(crate) fn from_group(group: &GroupInfo) -> Self {
        Self {
            group_id: group.group_id.clone(),
            name: group.name.clone(),
            owner: group.owner.clone(),
            members: group.members.clone(),
            rev: group.rev,
            ts_ms: group.ts_ms,
        }
    }

    /// 入站纵深校验：群名合法、owner 合法非本机、members 去重 ≤32 且含本机。
    pub(crate) fn validate(&self, local: p2p_identity::PeerId) -> Result<(), String> {
        validate_group_name(&self.name).map_err(|e| e.to_string())?;
        let owner = parse_peer_id(&self.owner).map_err(|e| e.to_string())?;
        if owner.to_string() == local.to_string() {
            return Err("roster owner 指向本机（本机管理的群不收 roster），拒收".into());
        }
        if self.members.len() > MAX_GROUP_MEMBERS {
            return Err(format!(
                "members 超过 {MAX_GROUP_MEMBERS} 上限：{}",
                self.members.len()
            ));
        }
        let unique: std::collections::HashSet<&String> = self.members.iter().collect();
        if unique.len() != self.members.len() {
            return Err("members 存在重复项，拒收".into());
        }
        if !self.members.contains(&local.to_string()) {
            return Err("members 不含本机，拒收".into());
        }
        Ok(())
    }
}

/// G_STATE_ACK（roster 事务回执）。
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupStateAck {
    pub(crate) group_id: String,
    pub(crate) rev: u64,
    pub(crate) ok: bool,
    pub(crate) reason: Option<String>,
}

/// G_KICK 单向通知（design §3 表）：reason = kicked | disbanded。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupKick {
    pub(crate) group_id: String,
    pub(crate) rev: u64,
    pub(crate) reason: String,
}

/// G_LEAVE 单向通知：sender = 退群成员（底座身份缺口，载荷声明发端，§3.1 同源）。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupLeave {
    pub(crate) group_id: String,
    pub(crate) sender: String,
}
