//! 启动期重询节奏（E4）：节点启动初期会错过 mDNS 公告/浏览窗口（组播丢包、
//! 同时启动、稳定期对端不重发 resolved），导致互发现延迟甚至失败；
//! 启动窗口内以短间隔主动重询补救，之后退到稳态周期。

use std::time::Duration;

/// 启动期重询间隔：短于默认通告周期（5s），保证窗口内至少一次重询命中。
const STARTUP_INTERVAL: Duration = Duration::from_secs(1);
/// 启动期重询次数：覆盖 >= 1 个完整通告周期的窗口。
const STARTUP_PROBES: u32 = 5;

/// 重询节奏：启动期 STARTUP_PROBES 次短间隔，之后固定稳态周期。
pub(crate) struct ProbeCadence {
    steady_interval: Duration,
    startup_left: u32,
}

impl ProbeCadence {
    pub(crate) fn new(steady_interval: Duration) -> Self {
        Self {
            steady_interval,
            startup_left: STARTUP_PROBES,
        }
    }

    /// 下一次重询前的等待时长。
    pub(crate) fn next(&mut self) -> Duration {
        if self.startup_left > 0 {
            self.startup_left -= 1;
            STARTUP_INTERVAL
        } else {
            self.steady_interval
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_burst_then_steady() {
        // E4 回归：启动窗口内必须以短间隔重询补错过的公告/浏览窗口，随后退到稳态
        let steady = Duration::from_secs(8);
        let mut cadence = ProbeCadence::new(steady);
        for i in 0..STARTUP_PROBES {
            assert_eq!(
                cadence.next(),
                STARTUP_INTERVAL,
                "第 {i} 次重询应为启动期短间隔"
            );
        }
        assert_eq!(cadence.next(), steady);
        assert_eq!(cadence.next(), steady, "启动期结束后固定稳态周期");
    }
}
