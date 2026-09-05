//! outbox/goutbox 双队列投递泵（design §6.2）：PeerConnected 统一触发，群队列复用同一纪律，分派走 group_core。
//! 群发送路径校验/信封构建在 group_send.rs（300 行红线再平衡）。

use std::sync::Arc;
use std::time::Duration;

use p2p::NodeEvent;
use tokio::sync::broadcast;

use crate::core::ChatCore;
use crate::group_core::GroupCore;
use crate::group_store::{GoutboxEntry, GoutboxFrame};
use crate::group_wire::{G_KICK, G_LEAVE};
use crate::model::{ChatError, ChatStatus};

const FLUSH_BATCH_CAP: usize = 32;

/// 启动补投首趟延迟：等监听与好友簿 rearm 就绪（对齐 invite heal 时序口径）。
const SWEEP_START_DELAY: Duration = Duration::from_secs(2);
/// 周期补投间隔：对端恢复可达后 pending 自动收敛的最坏感知时延。
const SWEEP_INTERVAL: Duration = Duration::from_secs(30);

/// 启动 + 周期补投泵（F3）：PeerConnected 泵只覆盖对端主动连入的窗口，
/// 本泵补齐「双方都有积压/我方重启」场景——遍历含积压对端主动拨号 flush，
/// 连接失败保持 pending 不计数（不可达窗口不消耗重试预算）。
pub(crate) fn spawn_outbox_sweeper(core: Arc<ChatCore>) {
    tokio::spawn(async move {
        tokio::time::sleep(SWEEP_START_DELAY).await;
        loop {
            for peer in core.store.outbox_peers() {
                if let Err(e) = flush_peer(&core, &peer).await {
                    tracing::warn!(%peer, error = %e, "outbox 周期补投失败");
                }
            }
            tokio::time::sleep(SWEEP_INTERVAL).await;
        }
    });
}

/// 死信阈值：条目硬失败（连接成功但流/协议失败）跨进程持久累计达 3 次即死信出队；
/// 连接失败与 unknown_group 不计数——不可达窗口不消耗预算（design §6.2 改版）。
const GOUTBOX_DEADLETTER_ATTEMPTS: u32 = 3;

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

/// 投递并统一记账：ACK 出队/记 acks；unknown_group 与连接失败保持 pending 不计数；
/// 硬失败标记 failed 并跨进程累计尝试次数（内联/后台共享同一落盘台账，design §6.2）。
pub(crate) async fn attempt(group: &GroupCore, entry: &GoutboxEntry) -> Result<bool, ChatError> {
    // 传输类失败（复用死亡连接的 mux closed 等）强拆重拨重试一次（同 1:1 deliver 纪律）
    let outcome = match dispatch_entry(group, entry).await {
        Ok(v) => Ok(v),
        Err(first @ ChatError::StreamFailed(_)) => {
            tracing::warn!(to = %entry.to, entry = %entry.id, error = %first, "群投递强拆重拨");
            group.redial(&entry.to).await?;
            dispatch_entry(group, entry).await
        }
        Err(e) => Err(e),
    };
    if let Err(e) = &outcome {
        if !matches!(e, ChatError::ConnectFailed(_)) {
            let attempts = entry.attempts + 1;
            tracing::warn!(
                to = %entry.to,
                entry = %entry.id,
                error = %e,
                attempts,
                cap = GOUTBOX_DEADLETTER_ATTEMPTS,
                "群投递硬失败，累计尝试次数"
            );
            if let Err(io) = group
                .store
                .mark_goutbox_failed(&entry.to, &entry.id, attempts)
            {
                tracing::warn!(to = %entry.to, entry = %entry.id, error = %io, "尝试计数落盘失败");
            }
        }
    }
    outcome
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
            tracing::warn!(to = %entry.to, entry = %entry.id, error = %e, "goutbox 条目投递失败");
            Err(e)
        }
    }
}

/// ACK 记账：出队条目；消息帧把成员计入 acks，全员确认时历史状态收敛 delivered
/// （磁盘与事件同源，design §4 状态机；实时路径的收敛在 finish_report）。
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
    let mut acks = group
        .store
        .history(&msg.group_id)
        .into_iter()
        .find(|m| m.id == msg.id)
        .map(|m| m.acks)
        .unwrap_or_default();
    if !acks.contains(&entry.to) {
        acks.push(entry.to.clone());
    }
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
    group
        .store
        .patch_message(&msg.group_id, &msg.id, |m| {
            m.acks = acks.clone();
            if delivered {
                m.status = ChatStatus::Delivered;
            }
        })
        .map_err(ChatError::Io)?;
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

/// goutbox flush（群侧纪律同 1:1）：批量上限内逐条投递；硬失败攒满即死信出队。
async fn flush_group_peer(group: &GroupCore, peer: &str) -> Result<(), ChatError> {
    let _guard = group.chat.peer_guard(peer).await;
    flush_entries(group, peer, None).await
}

/// 无锁补投体（调用方须已持该 peer 串行锁）：PeerConnected 泵与命令内补投共用，
/// 经条目落盘 attempts 共享同一尝试台账（同口径计数，无各自私账，消灭双路径竞态）。
/// skip = 命令内「先补积压再投新条」时排除本条，保证每条目每命令至多一次尝试。
pub(crate) async fn flush_entries(
    group: &GroupCore,
    peer: &str,
    skip: Option<&str>,
) -> Result<(), ChatError> {
    for entry in group
        .store
        .goutbox_for(peer)
        .into_iter()
        .take(FLUSH_BATCH_CAP)
    {
        if skip == Some(entry.id.as_str()) {
            continue;
        }
        if dead_letter_if_exhausted(group, peer, &entry) {
            continue;
        }
        attempt(group, &entry).await.ok();
    }
    Ok(())
}

/// 死信判定：硬失败次数跨进程持久累计（attempts 落盘）达上限即出队留告警；
/// 不可达窗口（连接失败）不计数，积压不在单进程内被死信（33df7e4 残余缺陷修法）。
fn dead_letter_if_exhausted(group: &GroupCore, peer: &str, entry: &GoutboxEntry) -> bool {
    if entry.attempts < GOUTBOX_DEADLETTER_ATTEMPTS {
        return false;
    }
    match group.store.remove_goutbox(peer, &entry.id) {
        Ok(()) => {
            tracing::warn!(
                to = %peer,
                entry = %entry.id,
                attempts = entry.attempts,
                "goutbox 条目重试耗尽，死信出队（历史记录保留）"
            );
            true
        }
        Err(e) => {
            tracing::warn!(to = %peer, entry = %entry.id, error = %e, "死信出队失败，保留条目");
            false
        }
    }
}
