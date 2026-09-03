//! 执行审计：每次工具调用产出结构化事件，挂入宿主钩子。
//!
//! 本批只留接缝与内存收集（[AuditSink]）；JSONL 落盘属 T26
//! （remote-support-plan.md §3.6：时间戳/tool/参数摘要/风险档/审批结果/结果摘要/耗时）。
//! 事件字段：时间戳/工具名/参数摘要/风险档/结果摘要/耗时，外加 outcome 区分
//! ok/denied/error。写失败必须留观测信号，禁止静默丢弃（本批内存收集，
//! 锁中毒即 error 日志）。

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// 一次工具调用的审计事件。
#[derive(Debug, Clone)]
pub struct AuditEvent {
    /// Unix 毫秒时间戳。
    pub at_unix_ms: u64,
    /// 工具名。
    pub tool: String,
    /// 参数摘要（compact JSON，截断）。
    pub params_summary: String,
    /// 风险档机器名（read/write/danger）。
    pub risk: String,
    /// 结果：ok | denied | error。
    pub outcome: String,
    /// 结果摘要（成功=结果文本头部，拒绝/失败=原因）。
    pub result_summary: String,
    /// 调用耗时（毫秒）。
    pub duration_ms: u64,
}

impl AuditEvent {
    pub fn new(
        tool: impl Into<String>,
        params_summary: impl Into<String>,
        risk: impl Into<String>,
        outcome: impl Into<String>,
        result_summary: impl Into<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            at_unix_ms: now_unix_ms(),
            tool: tool.into(),
            params_summary: params_summary.into(),
            risk: risk.into(),
            outcome: outcome.into(),
            result_summary: result_summary.into(),
            duration_ms,
        }
    }
}

/// 当前 Unix 毫秒时间戳（时钟回拨按 0 处理，不 panic）。
pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// 内存审计收集器（线程安全，Clone 共享同一收集）。
#[derive(Clone, Default)]
pub struct AuditSink {
    events: Arc<Mutex<Vec<AuditEvent>>>,
}

impl AuditSink {
    /// 追加一条事件；锁中毒不静默——留 error 日志并丢弃本条。
    pub fn push(&self, event: AuditEvent) {
        match self.events.lock() {
            Ok(mut guard) => guard.push(event),
            Err(_) => tracing::error!("audit sink lock poisoned; event dropped"),
        }
    }

    /// 快照全部事件（供 T26 JSONL 落盘与 session_report 消费）。
    pub fn snapshot(&self) -> Vec<AuditEvent> {
        self.events.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn len(&self) -> usize {
        self.events.lock().map(|g| g.len()).unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_fields_recorded() {
        let event = AuditEvent::new("fs_read", "{\"path\":\"a.txt\"}", "read", "ok", "hello", 3);
        assert_eq!(event.tool, "fs_read");
        assert_eq!(event.risk, "read");
        assert_eq!(event.outcome, "ok");
        assert_eq!(event.result_summary, "hello");
        assert_eq!(event.duration_ms, 3);
        assert!(
            event.at_unix_ms > 1_700_000_000_000,
            "timestamp sane: {}",
            event.at_unix_ms
        );
    }

    #[test]
    fn sink_push_snapshot_len() {
        let sink = AuditSink::default();
        let clone = sink.clone();
        assert!(sink.is_empty());
        clone.push(AuditEvent::new("fs_list", "{}", "read", "ok", "a", 1));
        sink.push(AuditEvent::new("sys_snapshot", "{}", "read", "ok", "b", 1));
        assert_eq!(sink.len(), 2);
        let events = sink.snapshot();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].tool, "fs_list");
        assert_eq!(events[1].tool, "sys_snapshot");
    }
}
