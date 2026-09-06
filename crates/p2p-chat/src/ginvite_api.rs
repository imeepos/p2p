//! 入群邀请（同意制）门面（Group 公共 API，group_ 前缀命名沿用）：
//! 发起 / 受邀者同意 / 受邀者拒绝 / 列表；卡片投递、离线重投与 roster 收敛联动
//! 在 ginvite_flow.rs（行数红线拆分）。

use crate::ginvite::{GroupInvite, GroupInviteDirection, GroupInviteState};
use crate::ginvite_flow::send_invite_card;
use crate::ginvite_wire::{deliver_frame, GinviteFrame, GACCEPT, GINVITE, GREJECT};
use crate::group_model::GroupResult;
use crate::group_store::{GroupInfo, MAX_GROUP_MEMBERS};
use crate::model::{now_ms, parse_peer_id, validate_nickname, ChatError, ChatFriend};

/// 发起结果：invite = 已落盘条目；delivered = 本轮邀请帧是否已送达受邀者
/// （false = 对端离线挂起，重连重投；卡片消息走 1:1 既有离线投递）。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInviteReport {
    pub invite: GroupInvite,
    pub delivered: bool,
}

impl crate::group::Group {
    /// 发起入群邀请（owner-only，同意制）：校验 → out 簿 pending → 邀请帧尽力
    /// 投递 → 1:1 卡片消息。重复邀请幂等刷新（state 回 pending、delivered 复位，
    /// 条目 id 稳定）；对端同意前群 roster 不变。
    pub async fn group_invite_member(
        &self,
        group_id: &str,
        invitee_id: &str,
        inviter_nickname: &str,
        note: Option<String>,
    ) -> GroupResult<GroupInviteReport> {
        let group = self.owned_group(group_id)?;
        let invitee = parse_peer_id(invitee_id)?;
        let local = self.core.chat.node.local_peer_id();
        if invitee == local {
            return Err(ChatError::SelfPeer(invitee_id.to_string()));
        }
        let friends = self.core.chat.store.friends_list()?;
        validate_invite_target(&group, &friends, invitee_id)?;
        let nickname = display_name(inviter_nickname, &local.to_string());
        let entry = self.refresh_or_create(
            group_id,
            group.name.clone(),
            &local.to_string(),
            invitee_id,
            note,
        );
        self.core.chat.store.upsert_group_invite(entry.clone())?;
        let _ = self
            .core
            .chat
            .events
            .send(crate::events::ChatEvent::GroupInvite {
                invite: entry.clone(),
            });
        let frame = GinviteFrame::new(&local, &entry, &nickname, None);
        let delivered = match deliver_frame(&self.core.chat, invitee_id, GINVITE, &frame).await {
            Ok(()) => {
                self.core
                    .chat
                    .store
                    .patch_group_invite(&entry.id, |i| i.delivered = true)?;
                true
            }
            Err(e) => {
                tracing::warn!(peer = %invitee_id, error = %e, "入群邀请帧投递失败，挂起待重连重投");
                false
            }
        };
        let card = crate::ginvite::ChatInviteCard {
            group_id: entry.group_id.clone(),
            group_name: entry.group_name.clone(),
            inviter_nickname: nickname,
            note: entry.note.clone(),
        };
        if let Err(e) = send_invite_card(&self.core.chat, invitee_id, card).await {
            tracing::warn!(peer = %invitee_id, error = %e, "入群邀请卡片发送失败，邀请保持挂起");
        }
        Ok(GroupInviteReport {
            invite: entry,
            delivered,
        })
    }

    /// 受邀者同意：决策落盘后回送 GACCEPT；owner 复用邀请入群路径推 roster，
    /// 受邀者收到含自己在列的 roster 后状态置 accepted（事件随收敛发出）。
    /// owner 离线时同意挂起（delivered=false），重连/重启重投。
    pub async fn group_invite_accept(&self, invite_id: &str) -> GroupResult<GroupInvite> {
        let entry = self.decidable_in_invite(invite_id, GroupInviteState::Rejected, "不可同意")?;
        self.core
            .chat
            .store
            .patch_group_invite(&entry.id, |i| i.delivered = false)?;
        self.reply_decision(&entry, GACCEPT, None).await;
        self.core
            .chat
            .store
            .find_group_invite(&entry.id)?
            .ok_or_else(|| ChatError::NotFound(format!("邀请不存在：{invite_id}")))
    }

    /// 受邀者拒绝：本机立即置 rejected（事件）并回送 GREJECT（可带理由）。
    pub async fn group_invite_reject(
        &self,
        invite_id: &str,
        reason: Option<String>,
    ) -> GroupResult<GroupInvite> {
        let entry = self.decidable_in_invite(invite_id, GroupInviteState::Accepted, "不可拒绝")?;
        let updated = self
            .core
            .chat
            .store
            .patch_group_invite(&entry.id, |i| {
                i.state = GroupInviteState::Rejected;
                i.delivered = false;
            })?
            .ok_or_else(|| ChatError::NotFound(format!("邀请不存在：{invite_id}")))?;
        crate::ginvite_flow::emit_decision(&self.core.chat, updated.clone());
        self.reply_decision(&updated, GREJECT, reason).await;
        Ok(updated)
    }

    /// 邀请列表（in + out 合一，tsMs 倒序）。
    pub fn group_invites_list(&self) -> GroupResult<Vec<GroupInvite>> {
        let mut list = self.core.chat.store.group_invites_list()?;
        list.sort_by_key(|i| std::cmp::Reverse(i.ts_ms));
        Ok(list)
    }

    /// out 条目刷新或新建（id 稳定，state/delivered 复位）。
    fn refresh_or_create(
        &self,
        group_id: &str,
        group_name: String,
        inviter: &str,
        invitee: &str,
        note: Option<String>,
    ) -> GroupInvite {
        let existing = self
            .core
            .chat
            .store
            .group_invites_list()
            .unwrap_or_default()
            .into_iter()
            .find(|i| {
                i.direction == GroupInviteDirection::Out
                    && i.group_id == group_id
                    && i.invitee == invitee
            });
        GroupInvite {
            id: existing
                .map(|i| i.id)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            group_id: group_id.to_string(),
            group_name,
            owner: inviter.to_string(),
            inviter: inviter.to_string(),
            invitee: invitee.to_string(),
            note,
            direction: GroupInviteDirection::Out,
            state: GroupInviteState::Pending,
            ts_ms: now_ms(),
            delivered: false,
        }
    }

    /// in 向待决策条目定位 + 状态守卫（illegal 目标态即显式 Err）。
    fn decidable_in_invite(
        &self,
        invite_id: &str,
        illegal: GroupInviteState,
        why: &str,
    ) -> GroupResult<GroupInvite> {
        let entry = self
            .core
            .chat
            .store
            .find_group_invite(invite_id)?
            .filter(|i| i.direction == GroupInviteDirection::In)
            .ok_or_else(|| ChatError::NotFound(format!("无待处理邀请：{invite_id}")))?;
        if entry.state == illegal {
            let name = match illegal {
                GroupInviteState::Accepted => "同意",
                GroupInviteState::Rejected => "拒绝",
                GroupInviteState::Pending => "待处理",
            };
            return Err(ChatError::InvalidUpdate(format!("该邀请已{name}，{why}")));
        }
        Ok(entry)
    }

    /// 决策帧回送：失败仅告警保持挂起（delivered=false），重连/重启重投收敛。
    async fn reply_decision(&self, entry: &GroupInvite, kind: u8, reason: Option<String>) {
        let frame = crate::ginvite_flow::decision_frame(&self.core.chat, entry, reason);
        if let Err(e) = deliver_frame(&self.core.chat, entry.counterparty(), kind, &frame).await {
            tracing::warn!(
                peer = %entry.counterparty(),
                id = %entry.id,
                error = %e,
                "决策帧回送失败，挂起待重连重投"
            );
        }
    }
}

/// 发起校验（纯函数便于单测）：群 active、受邀者不在群、群未满员、受邀者在好友簿。
pub(crate) fn validate_invite_target(
    group: &GroupInfo,
    friends: &[ChatFriend],
    invitee: &str,
) -> Result<(), ChatError> {
    if group.state != crate::group_store::GroupState::Active {
        return Err(ChatError::InvalidUpdate(format!(
            "群状态 {:?}，不可发起邀请",
            group.state
        )));
    }
    if group.members.iter().any(|m| m == invitee) {
        return Err(ChatError::InvalidUpdate(format!("已在群中：{invitee}")));
    }
    if group.members.len() >= MAX_GROUP_MEMBERS {
        return Err(ChatError::InvalidUpdate(format!(
            "群已满员（{MAX_GROUP_MEMBERS} 人上限）"
        )));
    }
    if !friends.iter().any(|f| f.peer_id == invitee) {
        return Err(ChatError::NotFound(format!("成员不在好友簿：{invitee}")));
    }
    Ok(())
}

/// 空昵称回退 PeerId 缩略（与 /im/invite/1 display_name 同口径）。
pub(crate) fn display_name(raw: &str, peer: &str) -> String {
    match validate_nickname(raw) {
        Ok(n) if !n.is_empty() => n,
        _ => crate::model::nickname_fallback(peer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group_store::GroupState;

    fn group(state: GroupState, member_count: usize) -> GroupInfo {
        let mut members = vec!["o".to_string()];
        for n in 1..member_count {
            members.push(format!("m{n}"));
        }
        GroupInfo {
            group_id: "g1".into(),
            name: "群".into(),
            owner: "o".into(),
            members,
            rev: 0,
            state,
            ts_ms: 1,
        }
    }

    fn friend(peer: &str) -> ChatFriend {
        ChatFriend {
            peer_id: peer.into(),
            nickname: "n".into(),
            addrs: vec![],
            note: None,
            group: None,
        }
    }

    #[test]
    fn invite_target_rejects_non_active_full_member_or_stranger() {
        let g = group(GroupState::Active, 1);
        assert!(validate_invite_target(&g, &[friend("e")], "e").is_ok());
        let err = validate_invite_target(&g, &[], "x").expect_err("非好友拒绝");
        assert!(err.to_string().contains("好友簿"), "err: {err}");
        let err = validate_invite_target(&g, &[friend("o")], "o").expect_err("已在群拒绝");
        assert!(err.to_string().contains("已在群中"), "err: {err}");
        let full = group(GroupState::Active, MAX_GROUP_MEMBERS);
        let err = validate_invite_target(&full, &[friend("e")], "e").expect_err("满员拒绝");
        assert!(err.to_string().contains("满员"), "err: {err}");
        let err = validate_invite_target(&group(GroupState::Disbanded, 1), &[friend("e")], "e")
            .expect_err("非 active 拒绝");
        assert!(err.to_string().contains("不可发起邀请"), "err: {err}");
    }

    #[test]
    fn empty_inviter_nickname_falls_back_to_peer_abbreviation() {
        assert_eq!(display_name("  群主甲 ", "abcdefgh1234"), "群主甲");
        assert_eq!(display_name("", "abcdefgh1234"), "abcdefgh...");
        assert_eq!(display_name(&"x".repeat(65), "abcdefgh1234"), "abcdefgh...");
    }
}
