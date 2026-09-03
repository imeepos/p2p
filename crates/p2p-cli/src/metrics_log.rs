//! 周期指标日志（E5）与 relay 指标渲染（E8-M2）：快照按固定节奏打进日志，
//! relay 快照另渲染为稳定 key=value 行供 stdout/采集脚本 grep。

use std::time::Duration;

use p2p_relay::RelayMetricsSnapshot;

/// 指标日志周期：P2P_METRICS_LOG_SECS 覆盖，缺省 60s，下限 5s。
pub fn log_interval() -> Duration {
    let secs = std::env::var("P2P_METRICS_LOG_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60)
        .max(5);
    Duration::from_secs(secs)
}

/// relay 快照渲染为稳定 key=value 行：键序固定，新增指标只追加不重排。
/// 解构全字段：快照加字段时此处编译失败，强制渲染面同步演进。
pub fn render_relay_metrics(s: &RelayMetricsSnapshot) -> String {
    let RelayMetricsSnapshot {
        circuits_active,
        circuits_bridged,
        circuits_issued_total,
        circuits_expired_total,
        circuits_recycled_total,
        bridges_idle_reclaimed_total,
        connect_rejects_total,
        keepalive_failures_total,
        bridged_bytes_total,
        links_active,
        controls_registered,
        reserve_rejects_total,
        link_rejects_total,
        punch_forwarded_total,
        punch_target_offline_total,
        punch_limited_total,
    } = *s;
    format!(
        "relay_circuits_active={circuits_active}\n\
         relay_circuits_bridged={circuits_bridged}\n\
         relay_circuits_issued_total={circuits_issued_total}\n\
         relay_circuits_expired_total={circuits_expired_total}\n\
         relay_circuits_recycled_total={circuits_recycled_total}\n\
         relay_bridges_idle_reclaimed_total={bridges_idle_reclaimed_total}\n\
         relay_connect_rejects_total={connect_rejects_total}\n\
         relay_reserve_rejects_total={reserve_rejects_total}\n\
         relay_link_rejects_total={link_rejects_total}\n\
         relay_keepalive_failures_total={keepalive_failures_total}\n\
         relay_bridged_bytes_total={bridged_bytes_total}\n\
         relay_links_active={links_active}\n\
         relay_controls_registered={controls_registered}\n\
         relay_punch_forwarded_total={punch_forwarded_total}\n\
         relay_punch_target_offline_total={punch_target_offline_total}\n\
         relay_punch_limited_total={punch_limited_total}\n"
    )
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

    #[test]
    fn render_is_greppable_key_value_lines() {
        let snap = RelayMetricsSnapshot {
            circuits_active: 3,
            bridged_bytes_total: 256,
            keepalive_failures_total: 1,
            ..RelayMetricsSnapshot::default()
        };
        let text = render_relay_metrics(&snap);
        assert!(text.contains("relay_circuits_active=3"));
        assert!(text.contains("relay_bridged_bytes_total=256"));
        assert!(text.contains("relay_keepalive_failures_total=1"));
        assert!(text.contains("relay_circuits_recycled_total=0"));
        assert_eq!(text.lines().count(), 16, "每指标一行");
    }
}
