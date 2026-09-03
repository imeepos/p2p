//! 全量查询应答的快照缓存：register 即失效、按条目真实 TTL 重建，
//! 把全量查询的 O(N) 克隆+编码从每查询一次削峰到每缓存窗口一次。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// 空表快照的兜底重建节奏（无真实 TTL 可依据；新注册会主动失效）。
pub(crate) const EMPTY_SNAPSHOT_RECHECK: Duration = Duration::from_secs(5);

struct SnapshotCache {
    encoded: Vec<u8>,
    valid_until: Instant,
}

type Entries = HashMap<String, SnapshotCache>;

/// 每 namespace 一份编码快照；写方 register 失效，查询路径重建。
pub(crate) struct SnapshotStore {
    entries: Mutex<Entries>,
    rebuilds: AtomicU64,
}

impl Default for SnapshotStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotStore {
    pub(crate) fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            rebuilds: AtomicU64::new(0),
        }
    }

    /// 命中返回编码帧；缺失/过期返回 None（调用方重建后 record）。
    pub(crate) fn get_fresh(&self, namespace: &str, now: Instant) -> Option<Vec<u8>> {
        Self::lock(&self.entries)
            .get(namespace)
            .filter(|c| c.valid_until > now)
            .map(|c| c.encoded.clone())
    }

    pub(crate) fn record(&self, namespace: &str, encoded: Vec<u8>, valid_until: Instant) {
        self.rebuilds.fetch_add(1, Ordering::Relaxed);
        Self::lock(&self.entries).insert(
            namespace.to_string(),
            SnapshotCache {
                encoded,
                valid_until,
            },
        );
    }

    /// 注册改变内容：对应 namespace 快照即刻失效。
    pub(crate) fn invalidate(&self, namespace: &str) {
        Self::lock(&self.entries).remove(namespace);
    }

    /// 快照重建次数（缓存命中情况的可观测信号，测试断言用）。
    pub(crate) fn rebuild_count(&self) -> u64 {
        self.rebuilds.load(Ordering::Relaxed)
    }

    fn lock(entries: &Mutex<Entries>) -> MutexGuard<'_, Entries> {
        entries.lock().unwrap_or_else(|p| p.into_inner())
    }
}
