//! 指数退避工具（design §8）：纯计算，不执行等待；重连决策留给业务。

use std::time::Duration;

/// 指数退避序列：base, base*2, base*4 ... 封顶 max。
#[derive(Debug, Clone)]
pub struct Backoff {
    base: Duration,
    max: Duration,
    attempts: u32,
}

impl Backoff {
    pub fn new(base: Duration, max: Duration) -> Self {
        Self {
            base,
            max,
            attempts: 0,
        }
    }

    /// 下一次退避时长；不睡眠，由调用方决定是否等待。
    pub fn next_delay(&mut self) -> Duration {
        let scale = 1u32 << self.attempts.min(20);
        let delay = self
            .base
            .checked_mul(scale)
            .unwrap_or(self.max)
            .min(self.max);
        self.attempts = self.attempts.saturating_add(1);
        delay
    }

    /// 连接成功后归零，退避序列从头开始。
    pub fn reset(&mut self) {
        self.attempts = 0;
    }

    pub fn attempts(&self) -> u32 {
        self.attempts
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new(Duration::from_millis(500), Duration::from_secs(30))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_then_caps() {
        let mut b = Backoff::new(Duration::from_millis(100), Duration::from_millis(800));
        assert_eq!(b.next_delay(), Duration::from_millis(100));
        assert_eq!(b.next_delay(), Duration::from_millis(200));
        assert_eq!(b.next_delay(), Duration::from_millis(400));
        assert_eq!(b.next_delay(), Duration::from_millis(800));
        assert_eq!(b.next_delay(), Duration::from_millis(800));
        assert_eq!(b.attempts(), 5);
    }

    #[test]
    fn reset_restarts_sequence() {
        let mut b = Backoff::new(Duration::from_millis(50), Duration::from_secs(10));
        b.next_delay();
        b.next_delay();
        b.reset();
        assert_eq!(b.attempts(), 0);
        assert_eq!(b.next_delay(), Duration::from_millis(50));
    }

    #[test]
    fn overflow_saturates_at_max() {
        let mut b = Backoff::new(Duration::from_secs(1), Duration::from_secs(60));
        for _ in 0..40 {
            assert!(b.next_delay() <= Duration::from_secs(60));
        }
    }
}
