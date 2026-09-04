//! outbox 重投纪律：PeerConnected 触发 flush；failed 条目每进程只给一次重投机会，
//! 再失败即死信出队（消息记录保留 failed，可观测告警）；批量上限约束 guard 占用时长。
//! 背景（跨机演练 D1/D4）：无界重投 failed 条目会在 peer 锁上饿死新发送，毒化节点外拨。

use std::sync::Arc;

use p2p::NodeEvent;
use tokio::sync::broadcast;

use crate::core::ChatCore;
use crate::model::ChatError;

/// 单次 flush 最多处理的条目数；超出部分等下一次连接事件继续。
const FLUSH_BATCH_CAP: usize = 32;

/// 监听 PeerConnected：触发该 peer 的 outbox flush（离线投递语义 §6.2）。
pub(crate) fn spawn_outbox_task(core: Arc<ChatCore>) {
    tokio::spawn(async move {
        let mut rx = core.node.events();
        loop {
            match rx.recv().await {
                Ok(NodeEvent::PeerConnected { peer }) => {
                    let peer_s = peer.to_string();
                    if let Err(e) = flush_peer(&core, &peer_s).await {
                        tracing::warn!(%peer, error = %e, "outbox flush 失败");
                    }
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
