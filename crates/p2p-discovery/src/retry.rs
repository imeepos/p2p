//! 有界重试原语（design §12：失败路径必须显式可观察，禁止静默）。
//!
//! 装配期一次性探测（地址观测等）对瞬时抖动敏感：单次随机失败即造成
//! 功能退化（ISSUE 2026-09-05 注册退化为 loopback）。本原语统一
//! 「有界次数 + 指数退避 + 耗尽显式报错」语义，供装配期探测复用。

use std::fmt;
use std::future::Future;
use std::time::Duration;

/// 重试策略：attempts 为总尝试次数（含首次，>=1）；interval 为首次失败后
/// 的等待，之后逐次翻倍、封顶 16×interval（有界退避防重试风暴）。
#[derive(Clone, Copy, Debug)]
pub struct RetryPolicy {
    pub attempts: u32,
    pub interval: Duration,
}

impl RetryPolicy {
    pub fn new(attempts: u32, interval: Duration) -> Self {
        Self {
            attempts: attempts.max(1),
            interval,
        }
    }

    /// 第 failed_attempt 次失败后的等待：1x、2x、4x…封顶 16x。
    fn delay_after(&self, failed_attempt: u32) -> Duration {
        let shift = failed_attempt.saturating_sub(1).min(4);
        self.interval * (1u32 << shift)
    }
}

/// 重试耗尽：总尝试次数与末次错误原文一并上抛，调用方留显式信号。
#[derive(Debug)]
pub struct RetryExhausted {
    pub attempts: u32,
    pub last_error: String,
}

impl fmt::Display for RetryExhausted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "retry exhausted after {} attempt(s); last error: {}",
            self.attempts, self.last_error
        )
    }
}

impl std::error::Error for RetryExhausted {}

/// 有界重试：op 失败按策略退避重试，任一次成功即返回；耗尽返回
/// [RetryExhausted]（attempts = 实际尝试次数）。
pub async fn retry_bounded<T, E, F, Fut>(
    policy: RetryPolicy,
    mut op: F,
) -> Result<T, RetryExhausted>
where
    E: fmt::Display,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut last_error = String::new();
    for attempt in 1..=policy.attempts {
        match op().await {
            Ok(value) => return Ok(value),
            Err(e) => {
                last_error = e.to_string();
                if attempt == policy.attempts {
                    break;
                }
                tokio::time::sleep(policy.delay_after(attempt)).await;
            }
        }
    }
    Err(RetryExhausted {
        attempts: policy.attempts,
        last_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// 注册退化场景回归（BASE1）：瞬时失败在预算内自愈，不退化不静默。
    #[tokio::test]
    async fn register_retry_transient_failure_recovers_within_budget() {
        let fails = Cell::new(0u32);
        let policy = RetryPolicy::new(3, Duration::from_millis(10));
        let result = retry_bounded(policy, || async {
            if fails.get() < 2 {
                fails.set(fails.get() + 1);
                Err("transient observation loss")
            } else {
                Ok("203.0.113.7:4000")
            }
        })
        .await;
        assert_eq!(
            result.expect("must recover within budget"),
            "203.0.113.7:4000"
        );
        assert_eq!(fails.get(), 2, "two transient failures retried");
    }

    /// 重试耗尽必须显式可观察：携带总尝试次数与末次错误，不静默吞掉。
    #[tokio::test]
    async fn register_retry_exhaustion_is_explicit_and_bounded() {
        let calls = Cell::new(0u32);
        let policy = RetryPolicy::new(3, Duration::from_millis(10));
        let result: Result<(), _> = retry_bounded(policy, || async {
            calls.set(calls.get() + 1);
            Err("observation refused")
        })
        .await;
        let err = result.expect_err("exhaustion must surface");
        assert_eq!(err.attempts, 3, "attempts bound must be honored");
        assert_eq!(calls.get(), 3);
        assert!(err.last_error.contains("observation refused"));
        assert!(err.to_string().contains("retry exhausted after 3"));
    }

    /// 退避节奏：延迟按 1x/2x/4x 翻倍且封顶 16x。
    #[test]
    fn retry_backoff_doubles_and_caps() {
        let policy = RetryPolicy::new(10, Duration::from_secs(1));
        assert_eq!(policy.delay_after(1), Duration::from_secs(1));
        assert_eq!(policy.delay_after(2), Duration::from_secs(2));
        assert_eq!(policy.delay_after(3), Duration::from_secs(4));
        assert_eq!(
            policy.delay_after(9),
            Duration::from_secs(16),
            "capped at 16x"
        );
    }
}
