//! 中继链路健康观测：RTT EMA + 负载水位，原子记账、读侧无锁。
//!
//! 数据源：keepalive 往返（RTT，对齐 TURN 客户端候选探测实践）与
//! Reserved/KeepAliveAck 捎带的 load_permille。经 Arc 共享给上层
//! 多中继选择器；陈旧快照即可驱动选择（power-of-two-choices 结论）。

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

/// 单条中继链路的健康快照。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelayHealthSnapshot {
    /// 往返时延 EMA（毫秒，向上取整）；尚未测量为 0。
    pub rtt_ema_ms: u64,
    /// 负载水位 permille（0..=1000）；尚未收到按 0（空闲）处理。
    pub load_permille: u32,
}

/// 可共享（Arc）的健康记账；写侧 keepalive/控制往返，读侧选择器。
/// RTT 以微秒记账：亚毫秒链路经 EMA 仍能以 1ms 的口径被观测到。
#[derive(Default)]
pub struct RelayHealth {
    rtt_ema_micros: AtomicU64,
    load_permille: AtomicU32,
}

impl RelayHealth {
    /// 整数 EMA：new = (3*old + sample)/4，单次采样权重 25%。
    fn next_ema(old: u64, sample: u64) -> u64 {
        (old * 3 + sample) / 4
    }

    /// 记一次往返（内部微秒粒度）。
    pub fn note_rtt(&self, rtt: Duration) {
        let sample = rtt.as_micros() as u64;
        let old = self.rtt_ema_micros.load(Ordering::Relaxed);
        self.rtt_ema_micros
            .store(Self::next_ema(old, sample), Ordering::Relaxed);
    }

    /// 记负载水位；越界值截断到 0..=1000。
    pub fn note_load(&self, permille: u32) {
        self.load_permille
            .store(permille.min(1000), Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> RelayHealthSnapshot {
        RelayHealthSnapshot {
            rtt_ema_ms: self.rtt_ema_micros.load(Ordering::Relaxed).div_ceil(1000),
            load_permille: self.load_permille.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ema_rises_toward_sample_and_falls_back() {
        assert_eq!(RelayHealth::next_ema(0, 100), 25);
        assert_eq!(RelayHealth::next_ema(25, 100), 43);
        assert_eq!(RelayHealth::next_ema(100, 0), 75, "采样下降同样收敛");
    }

    #[test]
    fn submillis_rtt_counts_as_one_ms() {
        let us = Duration::from_micros;
        let h = RelayHealth::default();
        h.note_rtt(us(50));
        h.note_rtt(us(70));
        assert_eq!(h.snapshot().rtt_ema_ms, 1, "亚毫秒链路 EMA 仍可观测");
    }

    #[test]
    fn millis_scale_rtt_reports_ceil_ms() {
        let h = RelayHealth::default();
        for _ in 0..20 {
            h.note_rtt(Duration::from_millis(30));
        }
        assert_eq!(h.snapshot().rtt_ema_ms, 30, "EMA 喂足采样后收敛到实际 RTT");
    }

    #[test]
    fn load_clamped_to_permille_range() {
        let h = RelayHealth::default();
        h.note_load(2000);
        assert_eq!(h.snapshot().load_permille, 1000);
        h.note_load(0);
        assert_eq!(h.snapshot().load_permille, 0);
    }
}
