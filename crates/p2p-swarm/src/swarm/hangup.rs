//! 本端主动挂断（GUI peer_disconnect 的底座路径）。
//!
//! 语义：仅从连接池出池并关闭该 peer 的在册连接，不做发现层清理；
//! PeerDisconnected 事件仍由 serve 循环在收流终止后统一发出，
//! 断开路径保持单出口（dial.rs serve_connection）。

use p2p_identity::PeerId;

use super::Swarm;

impl Swarm {
    /// 挂断与该 peer 的连接（幂等）：返回是否确有在册连接被关闭。
    pub fn disconnect(&self, peer: &PeerId) -> bool {
        let Some(mux) = self.pool.take(peer) else {
            return false;
        };
        tracing::info!(%peer, "peer disconnected by local hangup");
        mux.close();
        true
    }
}
