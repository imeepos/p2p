//! 带 TTL 的进程内地址缓存：last-known-good 降级的基础设施。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use p2p_identity::PeerId;
use p2p_transport::TransportAddr;

use crate::AddrCache;

/// [AddrCache] 的内存实现。put 覆盖旧条目；get/evict 只返回未过期条目，过期即清除。
pub struct MemCache {
    entries: Mutex<HashMap<PeerId, Entry>>,
}

struct Entry {
    addrs: Vec<TransportAddr>,
    expires_at: Instant,
}

impl MemCache {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// 取内部锁；中毒时恢复而非 panic（L3：网络路径零 expect）。
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<PeerId, Entry>> {
        self.entries.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// 返回全部未过期条目（并清除过期项），供 rendezvous 服务端应答全量查询。
    pub fn snapshot(&self) -> Vec<(PeerId, Vec<TransportAddr>)> {
        let now = Instant::now();
        let mut map = self.lock();
        map.retain(|_, e| e.expires_at > now);
        map.iter().map(|(p, e)| (*p, e.addrs.clone())).collect()
    }
}

impl Default for MemCache {
    fn default() -> Self {
        Self::new()
    }
}

impl AddrCache for MemCache {
    fn put(&self, peer: PeerId, addrs: Vec<TransportAddr>, ttl: Duration) {
        let entry = Entry {
            addrs,
            expires_at: Instant::now() + ttl,
        };
        self.lock().insert(peer, entry);
    }

    fn get(&self, peer: &PeerId) -> Option<Vec<TransportAddr>> {
        let now = Instant::now();
        let mut map = self.lock();
        let expired = matches!(map.get(peer), Some(e) if e.expires_at <= now);
        if expired {
            map.remove(peer);
            return None;
        }
        map.get(peer).map(|e| e.addrs.clone())
    }

    fn evict_expired(&self) -> Vec<PeerId> {
        let now = Instant::now();
        let mut map = self.lock();
        let expired: Vec<PeerId> = map
            .iter()
            .filter(|(_, e)| e.expires_at <= now)
            .map(|(p, _)| *p)
            .collect();
        for peer in &expired {
            map.remove(peer);
        }
        expired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(port: u16) -> Vec<TransportAddr> {
        vec![TransportAddr::Tcp {
            ip: "127.0.0.1".parse().unwrap(),
            port,
        }]
    }

    #[test]
    fn put_then_get() {
        let cache = MemCache::new();
        let peer = PeerId::from_bytes([1u8; 32]);
        cache.put(peer, addr(9000), Duration::from_secs(60));
        assert_eq!(cache.get(&peer), Some(addr(9000)));
    }

    #[test]
    fn ttl_expiry_clears_entry() {
        let cache = MemCache::new();
        let peer = PeerId::from_bytes([2u8; 32]);
        cache.put(peer, addr(9001), Duration::from_millis(10));
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(cache.get(&peer), None);
    }

    #[test]
    fn evict_expired_returns_and_clears() {
        let cache = MemCache::new();
        let live = PeerId::from_bytes([3u8; 32]);
        let dead = PeerId::from_bytes([4u8; 32]);
        cache.put(live, addr(1), Duration::from_secs(60));
        cache.put(dead, addr(2), Duration::from_millis(10));
        std::thread::sleep(Duration::from_millis(30));
        let evicted = cache.evict_expired();
        assert_eq!(evicted, vec![dead]);
        assert_eq!(cache.get(&live), Some(addr(1)));
        assert_eq!(cache.get(&dead), None);
    }

    #[test]
    fn snapshot_only_returns_live_entries() {
        let cache = MemCache::new();
        let live = PeerId::from_bytes([5u8; 32]);
        let dead = PeerId::from_bytes([6u8; 32]);
        cache.put(live, addr(3), Duration::from_secs(60));
        cache.put(dead, addr(4), Duration::from_millis(10));
        std::thread::sleep(Duration::from_millis(30));
        let snap = cache.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].0, live);
    }
}
