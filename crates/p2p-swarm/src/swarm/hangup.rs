//! 本端主动挂断（GUI peer_disconnect 的底座路径）。
//!
//! 语义：仅从连接池出池并关闭该 peer 的在册连接，不做发现层清理；
//! 出池先于关闭，serve 循环退出时 remove_if_same 不中，断开事件由本处
//! 直接补发（挂断方与对端各得一次 PeerDisconnected）。

use p2p_identity::PeerId;

use super::lifecycle::LifecycleMsg;
use super::Swarm;
use crate::lifecycle::LifecycleEvent;
use crate::{CloseReason, NodeEvent};

impl Swarm {
    /// 关停：停 accept 循环并断开全部在册连接。serve 循环经关停信号退出时
    /// 池已清空（remove_if_same 不中），断开事件由本处统一补发，GUI 侧
    /// 节点列表才能把对端翻成离线；E8 补发关闭原因归档（Local 档）。
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
        for peer in self.pool.clear() {
            tracing::debug!(%peer, "connection dropped on shutdown");
            self.emit(NodeEvent::PeerDisconnected { peer });
            let _ = self
                .lifecycle
                .events
                .send(LifecycleEvent::ConnectionClosed {
                    peer,
                    reason: CloseReason::Local,
                });
        }
    }

    /// 挂断与该 peer 的连接（幂等）：返回是否确有在册连接被关闭。
    pub fn disconnect(&self, peer: &PeerId) -> bool {
        let Some(mux) = self.pool.take(peer) else {
            return false;
        };
        tracing::info!(%peer, "peer disconnected by local hangup");
        mux.close();
        self.emit(NodeEvent::PeerDisconnected { peer: *peer });
        // E8：本端主动关闭归档 Local 档（与关停路径同口径，归因不缺档）
        let _ = self
            .lifecycle
            .events
            .send(LifecycleEvent::ConnectionClosed {
                peer: *peer,
                reason: CloseReason::Local,
            });
        // E6：挂断出册，停止自动重连（用户明确说再见，机器不再自作主张）
        self.lifecycle.notify(LifecycleMsg::HungUp { peer: *peer });
        true
    }
}
