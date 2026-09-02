//! 发现事件接线：DiscoveryEvent → 地址簿 + NodeEvent（design §7/§12）。

use std::sync::Arc;

use p2p_discovery::DiscoveryEvent;
use p2p_swarm::Swarm;
use tokio::sync::mpsc;

/// 转发发现事件：新地址入地址簿（触发 PeerDiscovered），
/// 过期发 PeerDisconnected（design §7.1），源失败留告警不打断其他源。
pub(crate) async fn forward_discovery(mut rx: mpsc::Receiver<DiscoveryEvent>, swarm: Arc<Swarm>) {
    while let Some(ev) = rx.recv().await {
        match ev {
            DiscoveryEvent::Discovered(dp) => swarm.add_peer_addresses(dp.peer, dp.addrs),
            DiscoveryEvent::Expired(peer) => swarm.on_peer_expired(peer),
            DiscoveryEvent::Failed { source, reason } => {
                tracing::warn!(source = ?source, %reason, "discovery source failed");
            }
        }
    }
}
