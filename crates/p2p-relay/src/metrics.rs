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
    /// 信令面消失回收的电路数（控制流关闭/链路归零，E6-R3 回收路径）。
    pub circuits_recycled_total: u64,
    /// 桥接空闲监管拆桥次数（E6-R3 空闲回收路径）。
    pub bridges_idle_reclaimed_total: u64,
    /// connect 阶段拒绝数（未知/过期/越权/超配额）。
    pub connect_rejects_total: u64,
    /// 服务端控制面保活失败：客户端静默超 server_silence 被清理的次数。
    pub keepalive_failures_total: u64,
    /// 桥接正常结束累计搬运的密文字节（双向之和；空闲拆桥的尾巴字节不计）。
    pub bridged_bytes_total: u64,
    pub links_active: u64,
    pub controls_registered: u64,
    pub reserve_rejects_total: u64,
    pub link_rejects_total: u64,
    /// 打洞信令成功转发条数（审查 M3 观测面）。
    pub punch_forwarded_total: u64,
    /// 目标不在线的信令条数。
    pub punch_target_offline_total: u64,
    /// 被限速拦下的信令条数。
    pub punch_limited_total: u64,
}

#[derive(Default)]
pub(crate) struct RelayMetrics {
    issued: AtomicU64,
    expired: AtomicU64,
    recycled: AtomicU64,
    idle_reclaimed: AtomicU64,
    connect_rejects: AtomicU64,
    keepalive_failures: AtomicU64,
    bridged_bytes: AtomicU64,
    reserve_rejects: AtomicU64,
    link_rejects: AtomicU64,
    punch_forwarded: AtomicU64,
    punch_offline: AtomicU64,
    punch_limited: AtomicU64,
}

impl RelayMetrics {
    pub(crate) fn count_issued(&self) {
        self.issued.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn count_expired(&self, n: u64) {
        self.expired.fetch_add(n, Ordering::Relaxed);
    }

    pub(crate) fn count_recycled(&self, n: u64) {
        self.recycled.fetch_add(n, Ordering::Relaxed);
    }

    pub(crate) fn count_idle_reclaimed(&self) {
        self.idle_reclaimed.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn count_connect_reject(&self) {
        self.connect_rejects.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn count_keepalive_failure(&self) {
        self.keepalive_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_bridged_bytes(&self, n: u64) {
        self.bridged_bytes.fetch_add(n, Ordering::Relaxed);
    }

    pub(crate) fn count_reserve_reject(&self) {
        self.reserve_rejects.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn count_link_reject(&self) {
        self.link_rejects.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn count_punch_forwarded(&self) {
        self.punch_forwarded.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn count_punch_offline(&self) {
        self.punch_offline.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn count_punch_limited(&self) {
        self.punch_limited.fetch_add(1, Ordering::Relaxed);
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
            circuits_recycled_total: load(&self.recycled),
            bridges_idle_reclaimed_total: load(&self.idle_reclaimed),
            connect_rejects_total: load(&self.connect_rejects),
            keepalive_failures_total: load(&self.keepalive_failures),
            bridged_bytes_total: load(&self.bridged_bytes),
            links_active: state.links.values().sum::<usize>() as u64,
            controls_registered: state.controls.len() as u64,
            reserve_rejects_total: load(&self.reserve_rejects),
            link_rejects_total: load(&self.link_rejects),
            punch_forwarded_total: load(&self.punch_forwarded),
            punch_target_offline_total: load(&self.punch_offline),
            punch_limited_total: load(&self.punch_limited),
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
        m.count_recycled(2);
        m.count_idle_reclaimed();
        m.count_connect_reject();
        m.count_keepalive_failure();
        m.add_bridged_bytes(128);
        m.count_reserve_reject();
        m.count_link_reject();
        m.count_punch_forwarded();
        m.count_punch_offline();
        m.count_punch_limited();
        let st = RelayState::new();
        let snap = m.snapshot(&st);
        assert_eq!(snap.circuits_issued_total, 2);
        assert_eq!(snap.circuits_expired_total, 1);
        assert_eq!(snap.circuits_recycled_total, 2);
        assert_eq!(snap.bridges_idle_reclaimed_total, 1);
        assert_eq!(snap.connect_rejects_total, 1);
        assert_eq!(snap.keepalive_failures_total, 1);
        assert_eq!(snap.bridged_bytes_total, 128);
        assert_eq!(snap.reserve_rejects_total, 1);
        assert_eq!(snap.link_rejects_total, 1);
        assert_eq!(snap.punch_forwarded_total, 1);
        assert_eq!(snap.punch_target_offline_total, 1);
        assert_eq!(snap.punch_limited_total, 1);
        assert_eq!(snap.circuits_active, 0);
        assert_eq!(snap.links_active, 0);
    }
}
