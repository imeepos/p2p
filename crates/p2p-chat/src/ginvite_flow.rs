//! 入群邀请收敛面：1:1 卡片发送、决策帧构造、PeerConnected/启动重投、
//! roster 收敛联动（受邀者 accepted 判定）。门面在 ginvite_api.rs。

use crate::events::ChatEvent;
use crate::ginvite::{GroupInvite, GroupInviteDirection, GroupInviteState};
use crate::ginvite_api::display_name;
use crate::ginvite_wire::{deliver_frame, GinviteFrame, GACCEPT, GINVITE, GREJECT};
use crate::group::GroupRoster;
use crate::model::{now_ms, parse_peer_id, ChatEnvelope, ChatError, ChatKind, ChatStatus, Sender};
use crate::ChatCore;

/// 入群邀请卡片（1:1，owner→受邀者）：构建 groupinvite 信封并复用 1:1 落盘/
/// 投递纪律（离线走 outbox）；公开 send() 显式拒绝该 kind（卡片仅随邀请流程发出）。
pub(crate) async fn send_invite_card(
    chat: &ChatCore,
    to: &str,
    card: crate::ginvite::ChatInviteCard,
) -> Result<(), ChatError> {
    let peer_id = parse_peer_id(to)?;
    if peer_id == chat.node.local_peer_id() {
        return Err(ChatError::SelfPeer(to.to_string()));
    }
    let env = ChatEnvelope {
        id: uuid::Uuid::new_v4().to_string(),
        peer: to.to_string(),
        sender: Sender::Me,
        kind: ChatKind::GroupInvite,
        ts_ms: now_ms(),
        text: None,
        media: None,
        status: ChatStatus::Pending,
        reply_to: None,
        card: Some(card),
    };
    chat.store.append_outbox(&env)?;
    chat.store.append_message(&env)?;
    chat.emit_status(to, &env.id, ChatStatus::Pending);
    let _guard = chat.peer_guard(to).await;
    match chat.deliver(&env).await {
        Ok(()) => chat.mark_delivered(to, &env).map(|_| ()),
        Err(ChatError::ConnectFailed(e)) => {
            tracing::warn!(peer = %to, id = %env.id, error = %e, "对端未连接，卡片消息保持 pending");
            Ok(())
        }
        Err(e) => {
            tracing::warn!(peer = %to, id = %env.id, error = %e, "卡片消息投递失败，标记 failed");
            chat.mark_failed(to, &env)?;
            Ok(())
        }
    }
}

/// 决策/邀请动作帧构造：inviter_nickname 从 1:1 卡片历史恢复（条目字段冻结
/// 不存昵称），缺失回退 inviter PeerId 缩略。
pub(crate) fn decision_frame(
    chat: &ChatCore,
    entry: &GroupInvite,
    reason: Option<String>,
) -> GinviteFrame {
    let local = chat.node.local_peer_id();
    let nickname = recover_inviter_nickname(chat, entry.counterparty(), &entry.group_id)
        .unwrap_or_else(|| display_name("", &entry.inviter));
    GinviteFrame::new(&local, entry, &nickname, reason)
}

/// 受邀者拒绝迁移事件（决策落盘即发，回送失败由重投收敛）。
pub(crate) fn emit_decision(chat: &ChatCore, invite: GroupInvite) {
    let _ = chat.events.send(ChatEvent::GroupInvite { invite });
}

/// PeerConnected 联动：重投该对端挂起帧——out 待送达邀请（GINVITE）与
/// in 待回送决策（rejected → GREJECT，其余 → GACCEPT）。
pub(crate) async fn flush_ginvites_peer(chat: &ChatCore, peer: &str) {
    let pending: Vec<GroupInvite> = match chat.store.group_invites_list() {
        Ok(list) => list
            .into_iter()
            .filter(|i| i.counterparty() == peer && !i.delivered)
            .collect(),
        Err(e) => {
            tracing::warn!(peer = %peer, error = %e, "入群邀请簿读取失败，跳过重投");
            return;
        }
    };
    for entry in pending {
        let kind = match entry.direction {
            GroupInviteDirection::Out => GINVITE,
            GroupInviteDirection::In if entry.state == GroupInviteState::Rejected => GREJECT,
            GroupInviteDirection::In => GACCEPT,
        };
        let frame = decision_frame(chat, &entry, None);
        match deliver_frame(chat, peer, kind, &frame).await {
            Ok(()) => {
                if let Err(e) = chat
                    .store
                    .patch_group_invite(&entry.id, |i| i.delivered = true)
                {
                    tracing::warn!(id = %entry.id, error = %e, "delivered 标记落盘失败");
                }
            }
            Err(e) => {
                tracing::warn!(peer = %peer, id = %entry.id, error = %e, "入群邀请挂起帧重投失败，保持挂起")
            }
        }
    }
}

/// 周期重投入口（outbox sweeper 联动）：对全部挂起帧的对端逐个 flush，
/// 覆盖「对端重启/我方决策后对端长期离线」窗口（同 outbox sweeper 节拍）。
pub(crate) async fn flush_ginvites_pending(chat: &ChatCore) {
    let Ok(list) = chat.store.group_invites_list() else {
        tracing::warn!("周期重投读取入群邀请簿失败");
        return;
    };
    let peers: Vec<String> = list
        .into_iter()
        .filter(|i| !i.delivered)
        .map(|i| i.counterparty().to_string())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    for peer in peers {
        flush_ginvites_peer(chat, &peer).await;
    }
}

/// 启动自愈：对全部挂起帧的对端逐个重投（拨号就绪时序不稳，退避三次，
/// 同 /im/invite/1 heal 口径）。
pub(crate) fn spawn_heal(core: std::sync::Arc<ChatCore>) {
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let Ok(list) = core.store.group_invites_list() else {
            tracing::warn!("启动自愈读取入群邀请簿失败");
            return;
        };
        let peers: Vec<String> = list
            .into_iter()
            .filter(|i| !i.delivered)
            .map(|i| i.counterparty().to_string())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        for peer in peers {
            for attempt in 0..3u32 {
                if attempt > 0 {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                }
                flush_ginvites_peer(&core, &peer).await;
                let pending = core
                    .store
                    .group_invites_list()
                    .unwrap_or_default()
                    .into_iter()
                    .any(|i| i.counterparty() == peer && !i.delivered);
                if !pending {
                    break;
                }
            }
        }
    });
}

/// roster 收敛联动（受邀者侧）：含本机在列的 roster 落定后，同群 in 向 pending
/// 邀请置 accepted（delivered 同步置位——owner 显然已处理同意帧）。
pub(crate) fn on_roster_applied(chat: &ChatCore, roster: &GroupRoster) {
    let local = chat.node.local_peer_id().to_string();
    if !roster.members.contains(&local) {
        return;
    }
    let Ok(list) = chat.store.group_invites_list() else {
        tracing::warn!(group_id = %roster.group_id, "roster 收敛联动读取邀请簿失败");
        return;
    };
    for entry in list.into_iter().filter(|i| {
        i.direction == GroupInviteDirection::In
            && i.state == GroupInviteState::Pending
            && i.group_id == roster.group_id
    }) {
        match chat.store.patch_group_invite(&entry.id, |i| {
            i.state = GroupInviteState::Accepted;
            i.delivered = true;
        }) {
            Ok(Some(updated)) => emit_decision(chat, updated),
            Ok(None) => {}
            Err(e) => tracing::warn!(id = %entry.id, error = %e, "roster 联动置 accepted 落盘失败"),
        }
    }
}

/// 卡片历史恢复 inviter 昵称：消息簿（对端会话）最近一条同群卡片为准。
fn recover_inviter_nickname(chat: &ChatCore, peer: &str, group_id: &str) -> Option<String> {
    let cards = chat.store.messages_for(peer).ok()?;
    cards
        .into_iter()
        .rev()
        .filter_map(|m| m.card)
        .find(|c| c.group_id == group_id)
        .map(|c| c.inviter_nickname)
}
