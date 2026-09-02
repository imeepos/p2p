//! 连接池：PeerId → 已升级连接，幂等 connect 与并发拨号合并（coordination.md S 包）。

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::sync::{Arc, Mutex};

use p2p_identity::PeerId;
use p2p_mux::MuxControl;
use tokio::sync::Mutex as AsyncMutex;

type Mux = Arc<dyn MuxControl>;

/// 单槽连接池：每 peer 至多一条在册连接；重复入池让位于先到者。
pub struct ConnectionPool {
    conns: Mutex<HashMap<PeerId, Mux>>,
    dial_locks: Mutex<HashMap<PeerId, Arc<AsyncMutex<()>>>>,
}

impl ConnectionPool {
    pub fn new() -> Self {
        Self {
            conns: Mutex::new(HashMap::new()),
            dial_locks: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, peer: &PeerId) -> Option<Mux> {
        self.conns.lock().expect("pool lock").get(peer).cloned()
    }

    /// 在册连接数（指标水位用）。
    pub fn len(&self) -> usize {
        self.conns.lock().expect("pool lock").len()
    }

    /// 是否无在册连接。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 幂等插入：peer 已在册时丢弃新连接并返回 false（先到者优先）。
    pub fn insert(&self, peer: PeerId, mux: Mux) -> bool {
        let mut conns = self.conns.lock().expect("pool lock");
        if conns.contains_key(&peer) {
            return false;
        }
        conns.insert(peer, mux);
        true
    }

    /// 仅当在册连接与 mux 同源时移除，防止误删重连后的新连接。
    pub fn remove_if_same(&self, peer: &PeerId, mux: &Mux) -> bool {
        let mut conns = self.conns.lock().expect("pool lock");
        match conns.get(peer) {
            Some(current) if Arc::ptr_eq(current, mux) => {
                conns.remove(peer);
                true
            }
            _ => false,
        }
    }

    /// 清空（关停用），返回被移除的 peer 列表。
    pub fn clear(&self) -> Vec<PeerId> {
        let mut conns = self.conns.lock().expect("pool lock");
        let peers = conns.keys().copied().collect();
        conns.clear();
        peers
    }

    /// 幂等获取或拨号：已有连接直接复用；同 peer 并发请求经拨号锁合并为一次拨号。
    pub async fn get_or_dial(
        &self,
        peer: PeerId,
        dial: impl Future<Output = io::Result<Mux>>,
    ) -> io::Result<Mux> {
        if let Some(mux) = self.get(&peer) {
            return Ok(mux);
        }
        let lock = self.dial_lock(&peer);
        let _guard = lock.lock().await;
        // 拿到拨号锁后复查：等锁期间他人可能已完成拨号
        if let Some(mux) = self.get(&peer) {
            return Ok(mux);
        }
        dial.await
    }

    fn dial_lock(&self, peer: &PeerId) -> Arc<AsyncMutex<()>> {
        self.dial_locks
            .lock()
            .expect("pool lock")
            .entry(*peer)
            .or_default()
            .clone()
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_mux::BoxedStream;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct StubMux;

    #[async_trait::async_trait]
    impl MuxControl for StubMux {
        async fn open_stream(&self) -> io::Result<BoxedStream> {
            Err(io::Error::other("stub mux"))
        }
        async fn accept_stream(&self) -> Option<BoxedStream> {
            None
        }
    }

    fn stub() -> Mux {
        Arc::new(StubMux)
    }

    #[test]
    fn insert_is_idempotent_per_peer() {
        let pool = ConnectionPool::new();
        let peer = PeerId::from_bytes([1; 32]);
        assert!(pool.insert(peer, stub()));
        assert!(!pool.insert(peer, stub()), "second insert must lose");
        assert!(pool.get(&peer).is_some());
    }

    #[test]
    fn remove_if_same_spares_replacement() {
        let pool = ConnectionPool::new();
        let peer = PeerId::from_bytes([2; 32]);
        let first = stub();
        let second = stub();
        assert!(pool.insert(peer, first.clone()));
        assert!(
            !pool.remove_if_same(&peer, &second),
            "must not remove other mux"
        );
        assert!(pool.remove_if_same(&peer, &first));
        assert!(!pool.remove_if_same(&peer, &first), "already removed");
    }

    /// 与 Swarm 的 dial_peer 契约一致：拨号成功方负责入池。
    async fn dial_once(
        pool: Arc<ConnectionPool>,
        peer: PeerId,
        counter: Arc<AtomicUsize>,
    ) -> io::Result<Mux> {
        counter.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let mux = stub();
        pool.insert(peer, mux.clone());
        Ok(mux)
    }

    #[tokio::test]
    async fn concurrent_get_or_dial_dials_once() {
        let pool = Arc::new(ConnectionPool::new());
        let peer = PeerId::from_bytes([3; 32]);
        let counter = Arc::new(AtomicUsize::new(0));
        let (r1, r2, r3) = tokio::join!(
            pool.get_or_dial(peer, dial_once(pool.clone(), peer, counter.clone())),
            pool.get_or_dial(peer, dial_once(pool.clone(), peer, counter.clone())),
            pool.get_or_dial(peer, dial_once(pool.clone(), peer, counter.clone())),
        );
        assert!(r1.is_ok() && r2.is_ok() && r3.is_ok());
        assert_eq!(counter.load(Ordering::SeqCst), 1, "dial must merge");
    }
}
