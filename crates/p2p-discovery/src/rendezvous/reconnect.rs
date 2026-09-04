//! 重连退避（E4 观测对齐 + libp2p 实践）：失败翻倍封顶，健康收尾复位。

use std::time::Duration;

/// 初值与上限：上限 30s 对齐服务端不可达时的探测节奏（E4 观测 ~35s 周期）。
const BACKOFF_INITIAL: Duration = Duration::from_millis(500);
const BACKOFF_MAX: Duration = Duration::from_secs(30);
/// 每步 ±20% 抖动：错开多节点重连相位，防重连风暴同步（libp2p 实践）。
const JITTER_PCT: f64 = 0.2;

/// 重连退避：失败逐次翻倍至上限；健康会话正常收尾即复位——
/// 退避只惩罚连续失败，长时间在线后一次断连不应等满上限（E4）。
pub(crate) struct ReconnectBackoff {
    initial: Duration,
    max: Duration,
    current: Duration,
}

impl ReconnectBackoff {
    pub(crate) fn new() -> Self {
        Self {
            initial: BACKOFF_INITIAL,
            max: BACKOFF_MAX,
            current: BACKOFF_INITIAL,
        }
    }

    /// 取本次等待时长（±20% 抖动）并翻倍推进（封顶 max）。
    pub(crate) fn step(&mut self) -> Duration {
        let wait = self.current;
        self.current = (self.current * 2).min(self.max);
        use rand::Rng;
        let pct = rand::thread_rng().gen_range(1.0 - JITTER_PCT..1.0 + JITTER_PCT);
        Duration::from_secs_f64(wait.as_secs_f64() * pct)
    }

    /// 健康会话收尾：退避复位到初值。
    pub(crate) fn reset(&mut self) {
        self.current = self.initial;
    }
}

#[cfg(test)]
mod tests;
