//! metrics_history 时间序列（契约 v2）：节点运行期每 5s 采样，环形保留 120 点。
//!
//! 采样任务随 node_start 起、stop 标志置位即退出；停止时序列清空（契约 §1 v2）。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use p2p::Node;
use p2p_swarm::MetricsSnapshot;
use serde::{Deserialize, Serialize};

use crate::util::now_ms;

/// 采样间隔：5s。
pub const SAMPLE_INTERVAL: Duration = Duration::from_secs(5);

/// 环形保留点数：120 点 × 5s = 10 分钟窗口。
pub const HISTORY_CAP: usize = 120;

/// 单个指标采样点（契约 v2 MetricsPoint）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsPoint {
    /// 采样时刻毫秒戳。
    pub t_ms: u64,
    pub active_connections: u64,
    pub relay_sessions_active: u64,
    /// 直连/打洞/中继三跳成功合计。
    pub dial_ok_total: u64,
    /// 三跳失败合计（不含 addr 级细分，避免与 hop 失败重复计数）。
    pub dial_fail_total: u64,
}

impl MetricsPoint {
    fn from_snapshot(snapshot: &MetricsSnapshot, t_ms: u64) -> Self {
        Self {
            t_ms,
            active_connections: snapshot.active_connections,
            relay_sessions_active: snapshot.relay_sessions_active,
            dial_ok_total: snapshot.dial_direct_ok
                + snapshot.dial_punch_ok
                + snapshot.dial_relay_ok,
            dial_fail_total: snapshot.dial_direct_fail
                + snapshot.dial_punch_fail
                + snapshot.dial_relay_fail,
        }
    }
}

#[derive(Default)]
struct HistoryInner {
    stopped: bool,
    points: VecDeque<MetricsPoint>,
}

/// 环形序列：写入与停止清空共用一把锁，消除停止瞬间的写清竞态。
#[derive(Default)]
pub(crate) struct MetricsHistory {
    inner: Mutex<HistoryInner>,
}

impl MetricsHistory {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 采样一点；已停止返回 false（采样任务据此退出）。
    pub(crate) fn sample(&self, point: MetricsPoint) -> bool {
        let mut inner = self.inner.lock().expect("指标序列锁中毒");
        if inner.stopped {
            return false;
        }
        if inner.points.len() == HISTORY_CAP {
            inner.points.pop_front();
        }
        inner.points.push_back(point);
        true
    }

    /// 节点停止：置停止标志并清空序列（契约：停止即清）。
    pub(crate) fn stop_and_clear(&self) {
        let mut inner = self.inner.lock().expect("指标序列锁中毒");
        inner.stopped = true;
        inner.points.clear();
    }

    /// 快照：时间升序；未运行/已清空返回空。
    pub(crate) fn snapshot(&self) -> Vec<MetricsPoint> {
        self.inner
            .lock()
            .expect("指标序列锁中毒")
            .points
            .iter()
            .copied()
            .collect()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.lock().expect("指标序列锁中毒").points.len()
    }
}

/// 起采样任务：立即采首点（趋势图有起点），此后每 5s 一点；
/// 节点停止后 sample 返回 false，任务自然退出，不持有 AppState。
pub(crate) fn spawn_metrics_sampler(node: Arc<Node>, history: Arc<MetricsHistory>) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SAMPLE_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let point = MetricsPoint::from_snapshot(&node.metrics(), now_ms());
            if !history.sample(point) {
                tracing::info!("节点已停止，指标采样任务退出");
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(t_ms: u64) -> MetricsPoint {
        MetricsPoint {
            t_ms,
            active_connections: 0,
            relay_sessions_active: 0,
            dial_ok_total: 0,
            dial_fail_total: 0,
        }
    }

    #[test]
    fn ring_buffer_caps_at_120_and_drops_oldest() {
        let history = MetricsHistory::new();
        for t in 0..150 {
            history.sample(point(t));
        }
        assert_eq!(history.len(), HISTORY_CAP);
        let snapshot = history.snapshot();
        assert_eq!(snapshot[0].t_ms, 30, "最老 30 点应被挤出");
        assert_eq!(snapshot[HISTORY_CAP - 1].t_ms, 149);
    }

    #[test]
    fn stop_and_clear_drops_points_and_rejects_late_samples() {
        let history = MetricsHistory::new();
        history.sample(point(1));
        history.sample(point(2));
        history.stop_and_clear();
        assert_eq!(history.len(), 0);
        assert!(!history.sample(point(3)), "停止后采样应被拒绝");
        assert!(history.snapshot().is_empty());
    }

    #[test]
    fn metrics_point_serializes_with_contract_field_names() {
        let point = MetricsPoint {
            t_ms: 1_700_000_000_000,
            active_connections: 3,
            relay_sessions_active: 1,
            dial_ok_total: 9,
            dial_fail_total: 4,
        };
        let encoded = serde_json::to_value(point).expect("序列化");
        let expected = serde_json::json!({
            "tMs": 1_700_000_000_000u64,
            "activeConnections": 3,
            "relaySessionsActive": 1,
            "dialOkTotal": 9,
            "dialFailTotal": 4,
        });
        assert_eq!(encoded, expected, "字段名须与契约 v2 逐字一致");
        let decoded: MetricsPoint = serde_json::from_value(encoded).expect("反序列化");
        assert_eq!(decoded, point);
    }

    #[test]
    fn point_totals_collapse_three_hops() {
        let snapshot = MetricsSnapshot {
            dial_direct_ok: 5,
            dial_direct_fail: 2,
            dial_punch_ok: 3,
            dial_relay_fail: 1,
            ..Default::default()
        };
        let point = MetricsPoint::from_snapshot(&snapshot, 7);
        assert_eq!(point.dial_ok_total, 8, "三跳成功相加");
        assert_eq!(point.dial_fail_total, 3, "三跳失败相加");
        assert_eq!(point.t_ms, 7);
    }
}
