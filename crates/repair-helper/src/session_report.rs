//! session_report 工具（read 档，remote-support-plan.md §3.5/§3.6）：
//! 导出当前工单的执行记录。消费 [AuditSink] 收集（含 JSONL 落盘后端），
//! 输出结构化 JSON，是第 4 步「交付与验收」执行记录的数据源。

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::audit::AuditSink;
use crate::{Tool, ToolResult};

/// 按工单导出执行记录：ticket_id + 事件清单（时间戳/工具/参数摘要/风险档/
/// 结果摘要/耗时/outcome）。
pub struct SessionReport {
    audit: AuditSink,
    ticket_id: String,
}

impl SessionReport {
    pub fn new(audit: AuditSink, ticket_id: impl Into<String>) -> Self {
        Self {
            audit,
            ticket_id: ticket_id.into(),
        }
    }
}

#[async_trait]
impl Tool for SessionReport {
    fn name(&self) -> &str {
        "session_report"
    }

    fn description(&self) -> &str {
        "导出当前工单的执行记录（结构化 JSON）"
    }

    async fn call(&self, _arguments: Value) -> Result<ToolResult, String> {
        let events = self.audit.snapshot();
        let text = json!({
            "ticketId": self.ticket_id,
            "count": events.len(),
            "events": events,
        })
        .to_string();
        Ok(ToolResult {
            text,
            truncated: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditEvent;

    #[tokio::test]
    async fn report_exports_snapshot_with_ticket_id() {
        let audit = AuditSink::default();
        audit.push(AuditEvent::new("fs_read", "{}", "read", "ok", "hi", 1));
        let tool = SessionReport::new(audit.clone(), "t-42");
        let result = tool.call(Value::Null).await.unwrap();
        let value: Value = serde_json::from_str(&result.text).unwrap();
        assert_eq!(value["ticketId"], "t-42");
        assert_eq!(value["count"], 1);
        assert_eq!(value["events"][0]["tool"], "fs_read");
        assert!(!result.truncated);
    }

    #[tokio::test]
    async fn report_is_empty_before_any_call() {
        let tool = SessionReport::new(AuditSink::default(), "t-0");
        let result = tool.call(Value::Null).await.unwrap();
        let value: Value = serde_json::from_str(&result.text).unwrap();
        assert_eq!(value["count"], 0);
    }
}
