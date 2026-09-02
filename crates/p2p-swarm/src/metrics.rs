//! 运行时指标（E5）：原子计数 + 快照读数，回答三问——拨号各跳成败、
//! 活跃连接/会话水位、重连次数。纯内存，不引入外部依赖与协议行为。

use std::sync::atomic::{AtomicU64, Ordering};

use crate::DialHop;

/// 快照：某时刻的累计计数与当前水位。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MetricsSnapshot {
    pub dial_direct_ok: u64,
    pub dial_direct_fail: u64,
    pub dial_punch_ok: u64,
    pub dial_punch_fail: u64,
    pub dial_relay_ok: u64,
    pub dial_relay_fail: u64,
    /// 直连跳内单个地址的失败次数（含 hairpin 短预算超时）。
    pub addr_dial_failures: u64,
    /// relay 会话重连动作次数（链路断开或注册失败后的重拨尝试）。
    pub relay_reconnects: u64,
    pub active_connections: u64,
    pub relay_sessions_active: u64,
}

impl MetricsSnapshot {
    /// 直连跳成功率：ok / (ok+fail)，无样本时返回 None。
    pub fn dial_direct_success_rate(&self) -> Option<f64> {
        success_rate(self.dial_direct_ok, self.dial_direct_fail)
    }

    /// 打洞跳成功率。
    pub fn dial_punch_success_rate(&self) -> Option<f64> {
        success_rate(self.dial_punch_ok, self.dial_punch_fail)
    }

    /// 中继跳成功率。
    pub fn dial_relay_success_rate(&self) -> Option<f64> {
        success_rate(self.dial_relay_ok, self.dial_relay_fail)
    }
}

fn success_rate(ok: u64, fail: u64) -> Option<f64> {
    let total = ok + fail;
    (total > 0).then(|| ok as f64 / total as f64)
}

/// 内部计数器：所有字段原子，Swarm 方法以 &self 更新。
#[derive(Default)]
pub(crate) struct Metrics {
    direct_ok: AtomicU64,
    direct_fail: AtomicU64,
    punch_ok: AtomicU64,
    punch_fail: AtomicU64,
    relay_ok: AtomicU64,
    relay_fail: AtomicU64,
    addr_dial_fail: AtomicU64,
    reconnects: AtomicU64,
}

impl Metrics {
    pub(crate) fn hop_ok(&self, hop: DialHop) {
        let cell = match hop {
            DialHop::Direct => &self.direct_ok,
            DialHop::Punch => &self.punch_ok,
            DialHop::Relay => &self.relay_ok,
        };
        cell.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn hop_fail(&self, hop: DialHop) {
        let cell = match hop {
            DialHop::Direct => &self.direct_fail,
            DialHop::Punch => &self.punch_fail,
            DialHop::Relay => &self.relay_fail,
        };
        cell.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn count_addr_dial_fail(&self) {
        self.addr_dial_fail.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn count_reconnect(&self) {
        self.reconnects.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn snapshot(&self, active_connections: u64, relay_sessions: u64) -> MetricsSnapshot {
        let load = |c: &AtomicU64| c.load(Ordering::Relaxed);
        MetricsSnapshot {
            dial_direct_ok: load(&self.direct_ok),
            dial_direct_fail: load(&self.direct_fail),
            dial_punch_ok: load(&self.punch_ok),
            dial_punch_fail: load(&self.punch_fail),
            dial_relay_ok: load(&self.relay_ok),
            dial_relay_fail: load(&self.relay_fail),
            addr_dial_failures: load(&self.addr_dial_fail),
            relay_reconnects: load(&self.reconnects),
            active_connections,
            relay_sessions_active: relay_sessions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hop_counters_are_isolated_per_kind_and_outcome() {
        let m = Metrics::default();
        m.hop_ok(DialHop::Direct);
        m.hop_fail(DialHop::Direct);
        m.hop_fail(DialHop::Punch);
        m.hop_ok(DialHop::Relay);
        let snap = m.snapshot(0, 0);
        assert_eq!(snap.dial_direct_ok, 1);
        assert_eq!(snap.dial_direct_fail, 1);
        assert_eq!(snap.dial_punch_fail, 1);
        assert_eq!(snap.dial_relay_ok, 1);
        assert_eq!(snap.dial_punch_ok, 0);
    }

    #[test]
    fn success_rate_none_without_samples_and_half_with_equal() {
        let m = Metrics::default();
        assert_eq!(m.snapshot(0, 0).dial_direct_success_rate(), None);
        m.hop_ok(DialHop::Direct);
        m.hop_fail(DialHop::Direct);
        let snap = m.snapshot(0, 0);
        let rate = snap.dial_direct_success_rate().expect("samples exist");
        assert!((rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn reconnect_and_addr_fail_counters_accumulate() {
        let m = Metrics::default();
        m.count_reconnect();
        m.count_reconnect();
        m.count_addr_dial_fail();
        let snap = m.snapshot(3, 1);
        assert_eq!(snap.relay_reconnects, 2);
        assert_eq!(snap.addr_dial_failures, 1);
        assert_eq!(snap.active_connections, 3);
        assert_eq!(snap.relay_sessions_active, 1);
    }
}
