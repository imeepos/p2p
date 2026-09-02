//! 周期指标日志（E5）：把运行时快照按固定节奏打进日志，供长稳采样解析。

use std::time::Duration;

/// 指标日志周期：P2P_METRICS_LOG_SECS 覆盖，缺省 60s，下限 5s。
pub fn log_interval() -> Duration {
    let secs = std::env::var("P2P_METRICS_LOG_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60)
        .max(5);
    Duration::from_secs(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_floor_is_five_seconds() {
        // 环境变量被测试进程污染时下限仍须成立
        let secs = std::env::var("P2P_METRICS_LOG_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok());
        if secs.is_none() {
            assert_eq!(log_interval(), Duration::from_secs(60));
        }
        assert!(log_interval() >= Duration::from_secs(5));
    }
}
