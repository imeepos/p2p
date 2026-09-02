//! relay 服务端指标（E5）：电路/链路水位读数与拒绝/发放累计，纯内存原子。

use std::sync::atomic::{AtomicU64, Ordering};

use crate::slots::CircuitPhase;
use crate::state::RelayState;

/// 服务端指标快照：gauges 为当前水位，*_total 为进程启动以来累计。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RelayMetricsSnapshot {
    pub circuits_active: u64,
    pub circuits_bridged: u64,
    pub circuits_issued_total: u64,
    pub circuits_expired_total: u64,
    pub links_active: u64,
    pub controls_registered: u64,
    pub reserve_rejects_total: u64,
    pub link_rejects_total: u64,
}

#[derive(Default)]
pub(crate) struct RelayMetrics {
    issued: AtomicU64,
    expired: AtomicU64,
    reserve_rejects: AtomicU64,
    link_rejects: AtomicU64,
}

impl RelayMetrics {
    pub(crate) fn count_issued(&self) {
        self.issued.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn count_expired(&self, n: u64) {
        self.expired.fetch_add(n, Ordering::Relaxed);
    }

    pub(crate) fn count_reserve_reject(&self) {
        self.reserve_rejects.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn count_link_reject(&self) {
        self.link_rejects.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn snapshot(&self, state: &RelayState) -> RelayMetricsSnapshot {
        let load = |c: &AtomicU64| c.load(Ordering::Relaxed);
        let bridged = state
            .circuits
            .values()
            .filter(|s| s.phase == CircuitPhase::Bridged)
            .count() as u64;
        RelayMetricsSnapshot {
            circuits_active: state.circuits.len() as u64,
            circuits_bridged: bridged,
            circuits_issued_total: load(&self.issued),
            circuits_expired_total: load(&self.expired),
            links_active: state.links.values().sum::<usize>() as u64,
            controls_registered: state.controls.len() as u64,
            reserve_rejects_total: load(&self.reserve_rejects),
            link_rejects_total: load(&self.link_rejects),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_accumulate_and_snapshot_reflects_watermarks() {
        let m = RelayMetrics::default();
        m.count_issued();
        m.count_issued();
        m.count_expired(1);
        m.count_reserve_reject();
        m.count_link_reject();
        let st = RelayState::new();
        let snap = m.snapshot(&st);
        assert_eq!(snap.circuits_issued_total, 2);
        assert_eq!(snap.circuits_expired_total, 1);
        assert_eq!(snap.reserve_rejects_total, 1);
        assert_eq!(snap.link_rejects_total, 1);
        assert_eq!(snap.circuits_active, 0);
        assert_eq!(snap.links_active, 0);
    }
}
