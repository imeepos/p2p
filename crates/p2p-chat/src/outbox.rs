//! outbox/goutbox 双队列投递泵（design §6.2）：PeerConnected 统一触发，群队列复用同一纪律，分派走 group_core。
//! 群发送路径校验/信封构建在 group_send.rs（300 行红线再平衡）。

use std::sync::Arc;

use p2p::NodeEvent;
use tokio::sync::broadcast;

use crate::core::ChatCore;
use crate::group_core::GroupCore;
use crate::group_store::{GoutboxEntry, GoutboxFrame};
use crate::group_wire::{G_KICK, G_LEAVE};
use crate::model::{ChatError, ChatStatus};

const FLUSH_BATCH_CAP: usize = 32;

/// 监听 PeerConnected：依次触发 1:1 outbox / 群 goutbox / 邀请重投。
pub(crate) fn spawn_outbox_task(core: Arc<ChatCore>, group: Arc<GroupCore>) {
    tokio::spawn(async move {
        let mut rx = core.node.events();
        loop {
            match rx.recv().await {
                Ok(NodeEvent::PeerConnected { peer }) => {
                    let peer_s = peer.to_string();
                    if let Err(e) = flush_peer(&core, &peer_s).await {
                        tracing::warn!(%peer, error = %e, "outbox flush 失败");
                    }
                    if let Err(e) = flush_group_peer(&group, &peer_s).await {
                        tracing::warn!(%peer, error = %e, "群 outbox flush 失败");
                    }
                    crate::invite_api::flush_invites_peer(&core, &peer_s).await;
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

/// 对端重连时重发该 peer 的 outbox 条目；连接失败保持 pending，其余标记 failed。
/// 持有 peer 串行锁：与 send 并发时等待其完成，避免同连接并发开流（上游缺陷）。
pub(crate) async fn flush_peer(core: &ChatCore, peer: &str) -> Result<(), ChatError> {
    let _guard = core.peer_guard(peer).await;
    for env in core
        .store
        .outbox_for(peer)
        .into_iter()
        .take(FLUSH_BATCH_CAP)
    {
        if core.dead_letter_if_spent(peer, &env) {
            continue;
        }
        match core.deliver(&env).await {
            Ok(()) => core.mark_delivered(peer, &env)?,
            Err(ChatError::ConnectFailed(e)) => {
                tracing::warn!(peer = %peer, id = %env.id, error = %e, "flush 连接失败，保持 pending");
            }
            Err(e) => {
                tracing::warn!(peer = %peer, id = %env.id, error = %e, "flush 重发失败，标记 failed");
                core.mark_attempted(peer, &env.id);
                core.mark_failed(peer, &env)?;
            }
        }
    }
    Ok(())
}

/// 投递并记账：ACK 出队/记 acks；unknown_group 与连接失败保持 pending；硬失败标记 failed。
pub(crate) async fn attempt(group: &GroupCore, entry: &GoutboxEntry) -> Result<bool, ChatError> {
    // 传输类失败（复用死亡连接的 mux closed 等）强拆重拨重试一次（同 1:1 deliver 纪律）
    match dispatch_entry(group, entry).await {
        Ok(v) => Ok(v),
        Err(first @ ChatError::StreamFailed(_)) => {
            tracing::warn!(to = %entry.to, entry = %entry.id, error = %first, "群投递强拆重拨");
            group.redial(&entry.to).await?;
            dispatch_entry(group, entry).await
        }
        Err(e) => Err(e),
    }
}

async fn dispatch_entry(group: &GroupCore, entry: &GoutboxEntry) -> Result<bool, ChatError> {
    let acked = match &entry.frame {
        GoutboxFrame::Msg { msg } => group.send_msg(&entry.to, msg).await,
        GoutboxFrame::Roster { roster } => group.send_roster(&entry.to, roster).await.map(|_| true),
        GoutboxFrame::Kick { kick } => group
            .send_notice(&entry.to, G_KICK, kick)
            .await
            .map(|_| true),
        GoutboxFrame::Leave { leave } => group
            .send_notice(&entry.to, G_LEAVE, leave)
            .await
            .map(|_| true),
    };
    match acked {
        Ok(true) => {
            on_acked(group, entry)?;
            Ok(true)
        }
        Ok(false) => {
            tracing::warn!(to = %entry.to, entry = %entry.id, "对端缺群 unknown_group，条目保持 pending 等 roster");
            Ok(false)
        }
        Err(e @ ChatError::ConnectFailed(_)) => {
            tracing::warn!(to = %entry.to, entry = %entry.id, error = %e, "连接失败，条目保持 pending");
            Err(e)
        }
        Err(e) => {
            tracing::warn!(to = %entry.to, entry = %entry.id, error = %e, "goutbox 条目投递失败，标记 failed");
            group
                .store
                .set_goutbox_status(&entry.to, &entry.id, ChatStatus::Failed)
                .map_err(ChatError::Io)?;
            Err(e)
        }
    }
}

/// ACK 记账：出队条目；消息帧追加 acks，按「覆盖当前其他全体成员」判定 delivered。
fn on_acked(group: &GroupCore, entry: &GoutboxEntry) -> Result<(), ChatError> {
    let GoutboxFrame::Msg { msg } = &entry.frame else {
        return group
            .store
            .remove_goutbox(&entry.to, &entry.id)
            .map_err(ChatError::Io);
    };
    group
        .store
        .remove_goutbox(&entry.to, &entry.id)
        .map_err(ChatError::Io)?;
    group
        .store
        .patch_message(&msg.group_id, &msg.id, |m| {
            if !m.acks.contains(&entry.to) {
                m.acks.push(entry.to.clone());
            }
        })
        .map_err(ChatError::Io)?;
    let acks = group
        .store
        .history(&msg.group_id)
        .into_iter()
        .find(|m| m.id == msg.id)
        .map(|m| m.acks)
        .unwrap_or_default();
    let local = group.chat.node.local_peer_id().to_string();
    let delivered = group
        .store
        .group(&msg.group_id)
        .map(|g| {
            g.members
                .iter()
                .filter(|m| **m != local)
                .all(|m| acks.contains(m))
        })
        .unwrap_or(false);
    group.emit(crate::group_model::GroupEvent::Status {
        group_id: msg.group_id.clone(),
        message_id: msg.id.clone(),
        acks,
        status: if delivered {
            ChatStatus::Delivered
        } else {
            ChatStatus::Pending
        },
    });
    Ok(())
}

/// goutbox flush（群侧纪律同 1:1）：批量上限内逐条投递；failed 重投一次未愈即死信出队。
async fn flush_group_peer(group: &GroupCore, peer: &str) -> Result<(), ChatError> {
    let _guard = group.chat.peer_guard(peer).await;
    for entry in group
        .store
        .goutbox_for(peer)
        .into_iter()
        .take(FLUSH_BATCH_CAP)
    {
        if dead_letter_if_spent(group, peer, &entry) {
            continue;
        }
        if attempt(group, &entry).await.is_err() {
            group
                .flush_tried
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert((peer.to_string(), entry.id.clone()), ());
        }
    }
    Ok(())
}

/// failed 条目是否已在本进程重投过：是则死信出队（历史记录保留）并返回 true。
fn dead_letter_if_spent(group: &GroupCore, peer: &str, entry: &GoutboxEntry) -> bool {
    if entry.status != ChatStatus::Failed {
        return false;
    }
    let key = (peer.to_string(), entry.id.clone());
    if !group
        .flush_tried
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains_key(&key)
    {
        return false;
    }
    match group.store.remove_goutbox(peer, &entry.id) {
        Ok(()) => {
            tracing::warn!(to = %peer, entry = %entry.id, "goutbox 条目重投未愈，死信出队");
            true
        }
        Err(e) => {
            tracing::warn!(to = %peer, entry = %entry.id, error = %e, "死信出队失败，保留条目");
            false
        }
    }
}
