//! 重复发现的限频重发门：地址无新增的发现刷新按窗口放行重发一次
//! PeerDiscovered，供上层把「最后活跃」推进到当前（地址去重后后端
//! 原本不再发事件，在线但地址不变的节点会被误标离线）。

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use p2p_identity::PeerId;

/// 默认重发窗口：每 peer 至多每 60s 一条，事件量有界。
pub(crate) const REFRESH_EMIT_INTERVAL: Duration = Duration::from_secs(60);

pub(crate) struct RefreshGate {
    interval: Mutex<Duration>,
    stamps: Mutex<HashMap<PeerId, Instant>>,
}

impl Default for RefreshGate {
    fn default() -> Self {
        Self::new(REFRESH_EMIT_INTERVAL)
    }
}

impl RefreshGate {
    pub(crate) fn new(interval: Duration) -> Self {
        Self {
            interval: Mutex::new(interval),
            stamps: Mutex::new(HashMap::new()),
        }
    }

    /// 距上次重发超过窗口才放行（放行即记账本次时刻）。
    pub(crate) fn allows(&self, peer: PeerId, now: Instant) -> bool {
        let window = *Self::lock(&self.interval);
        let mut stamps = Self::lock(&self.stamps);
        match stamps.get(&peer) {
            Some(t) if now.duration_since(*t) < window => false,
            _ => {
                stamps.insert(peer, now);
                true
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn set_interval(&self, window: Duration) {
        *Self::lock(&self.interval) = window;
    }

    fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
        m.lock().unwrap_or_else(|p| p.into_inner())
    }
}
