//! 本端主动挂断（GUI peer_disconnect 的底座路径）。
//!
//! 语义：仅从连接池出池并关闭该 peer 的在册连接，不做发现层清理；
//! 出池先于关闭，serve 循环退出时 remove_if_same 不中，断开事件由本处
//! 直接补发（挂断方与对端各得一次 PeerDisconnected）。

use p2p_identity::PeerId;

use super::lifecycle::LifecycleMsg;
use super::Swarm;
use crate::NodeEvent;

impl Swarm {
    /// 挂断与该 peer 的连接（幂等）：返回是否确有在册连接被关闭。
    pub fn disconnect(&self, peer: &PeerId) -> bool {
        let Some(mux) = self.pool.take(peer) else {
            return false;
        };
        tracing::info!(%peer, "peer disconnected by local hangup");
        mux.close();
        self.emit(NodeEvent::PeerDisconnected { peer: *peer });
        // E6：挂断出册，停止自动重连（用户明确说再见，机器不再自作主张）
        self.lifecycle.notify(LifecycleMsg::HungUp { peer: *peer });
        true
    }
}
