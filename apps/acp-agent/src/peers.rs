//! peer 归属账本：facade 的 ProtocolHandler 只给流不给 PeerId（crates/p2p 契约缺口），
//! 桥以 Node 事件维护在线 peer 集，入站流归属规则：
//! 恰有一个在线 peer -> 归属之；零个/多个 -> 拒绝归属（fail-closed，审计可见）。
//! 多 peer 并发控制台需底座在流分发层暴露 peer（后续 crates/p2p 卡解除）。

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use p2p::{NodeEvent, PeerId};

#[derive(Default)]
pub struct PeerBook {
    peers: Mutex<Vec<PeerId>>,
}

impl PeerBook {
    /// 订阅节点事件并后台维护在线集；必须在任何连接建立前 spawn。
    pub fn spawn(mut events: tokio::sync::broadcast::Receiver<NodeEvent>) -> Arc<Self> {
        let book = Arc::new(Self::default());
        let worker = book.clone();
        tokio::spawn(async move {
            loop {
                match events.recv().await {
                    Ok(NodeEvent::PeerConnected { peer }) => worker.insert(peer),
                    Ok(NodeEvent::PeerDisconnected { peer }) => worker.remove(&peer),
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "peer book lagged; online set may be stale");
                    }
                }
            }
        });
        book
    }

    fn insert(&self, peer: PeerId) {
        let mut peers = self.lock();
        if !peers.contains(&peer) {
            peers.push(peer);
        }
    }

    fn remove(&self, peer: &PeerId) {
        self.lock().retain(|p| p != peer);
    }

    pub fn connected(&self) -> Vec<PeerId> {
        self.lock().clone()
    }

    /// 归属入站流 peer：单在线 peer 直接归属；空集允许短暂等待事件补账；
    /// 多 peer 歧义立即失败（安全边界宁可误拒不可误认）。
    pub async fn resolve(&self, wait: Duration) -> Option<PeerId> {
        let deadline = Instant::now() + wait;
        loop {
            let peers = self.connected();
            match peers.len() {
                1 => return peers.into_iter().next(),
                0 => {
                    if Instant::now() >= deadline {
                        return None;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                _ => {
                    tracing::warn!(
                        online = peers.len(),
                        "peer attribution ambiguous; denying (fail-closed)",
                    );
                    return None;
                }
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<PeerId>> {
        self.peers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_is_idempotent_and_remove_works() {
        let book = PeerBook::default();
        let key = [7u8; 32];
        let peer = p2p::PeerId::from_bytes(key);
        book.insert(peer);
        book.insert(peer);
        assert_eq!(book.connected().len(), 1);
        book.remove(&peer);
        assert!(book.connected().is_empty());
    }

    #[test]
    fn empty_online_set_times_out() {
        let book = PeerBook::default();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");
        let got = rt.block_on(book.resolve(Duration::from_millis(30)));
        assert!(got.is_none());
    }
}
