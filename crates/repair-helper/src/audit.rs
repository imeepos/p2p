//! 执行审计：每次工具调用产出结构化事件，挂入宿主钩子。
//!
//! 事件字段：时间戳/工具名/参数摘要/风险档/结果摘要/耗时，外加 outcome 区分
//! ok/denied/error（remote-support-plan.md §3.6 执行记录）。
//! T26 起支持文件后端：[AuditSink::with_file] 单文件 + 启动截断，逐调用一行
//! JSON 追加写（序列化为 camelCase）。写失败必须留 error 日志信号（禁止静默
//! 丢弃），文件后端随后转为关闭态、退化为内存收集，session_report 不受影响。

use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// 一次工具调用的审计事件。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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

/// 内存审计收集器（线程安全，Clone 共享同一收集）；可选 JSONL 文件后端。
#[derive(Clone, Debug)]
pub struct AuditSink {
    events: Arc<Mutex<Vec<AuditEvent>>>,
    /// 文件后端：Some(打开态) / None(未配置或写失败已关闭)。
    file: Option<Arc<Mutex<Option<BufWriter<File>>>>>,
}

impl Default for AuditSink {
    fn default() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            file: None,
        }
    }
}

impl AuditSink {
    /// 文件后端：单文件 + 启动截断（每次 helper 启动即新一轮审计）。
    /// 打开失败原样上抛（启动期即响亮失败，禁止静默降级）。
    pub fn with_file(path: &Path) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        Ok(Self {
            events: Arc::new(Mutex::new(Vec::new())),
            file: Some(Arc::new(Mutex::new(Some(BufWriter::new(file))))),
        })
    }

    /// 追加一条事件：先入内存收集（session_report 消费），再落 JSONL。
    /// 锁中毒不静默——留 error 日志并丢弃本条。
    pub fn push(&self, event: AuditEvent) {
        match self.events.lock() {
            Ok(mut guard) => guard.push(event.clone()),
            Err(_) => {
                tracing::error!("audit sink lock poisoned; event dropped");
                return;
            }
        }
        if self.file.is_some() {
            self.write_line(&event);
        }
    }

    /// JSONL 追加写：单行一条；写/刷失败留 error 日志并关闭文件后端
    /// （退化为内存收集，后续不再重复刷错误日志）。
    fn write_line(&self, event: &AuditEvent) {
        let line = match serde_json::to_string(event) {
            Ok(line) => line,
            Err(e) => {
                tracing::error!(%e, "audit event serialization failed; event not persisted");
                return;
            }
        };
        let mut slot = match self.file.as_ref().and_then(|f| f.lock().ok()) {
            Some(slot) => slot,
            None => {
                tracing::error!("audit file lock poisoned; event not persisted");
                return;
            }
        };
        let Some(writer) = slot.as_mut() else {
            tracing::debug!("audit file backend already disabled; memory collection only");
            return;
        };
        if let Err(e) = writeln!(writer, "{line}") {
            tracing::error!(%e, "audit file write failed; file backend disabled");
            *slot = None;
            return;
        }
        if let Err(e) = writer.flush() {
            tracing::error!(%e, "audit file flush failed; file backend disabled");
            *slot = None;
        }
    }

    /// 快照全部事件（供 session_report 消费）。
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

    fn temp_jsonl(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("rh-audit-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        (dir.join("audit.jsonl"), dir)
    }

    #[test]
    fn file_backend_writes_one_json_line_per_event() {
        let (path, _dir) = temp_jsonl("lines");
        let sink = AuditSink::with_file(&path).unwrap();
        sink.push(AuditEvent::new("fs_read", "{}", "read", "ok", "x", 1));
        let clone = sink.clone();
        clone.push(AuditEvent::new("fs_list", "{}", "read", "denied", "no", 2));
        drop(clone);
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "one JSON per event: {content}");
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["tool"], "fs_read");
        assert!(first["atUnixMs"].as_u64().unwrap() > 1_700_000_000_000);
        let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(second["tool"], "fs_list");
        assert_eq!(second["outcome"], "denied");
        // 内存收集仍可用（session_report 数据源）
        assert_eq!(sink.len(), 2);
    }

    #[test]
    fn file_backend_truncates_on_open() {
        let (path, _dir) = temp_jsonl("trunc");
        std::fs::write(&path, "stale line from previous run\n").unwrap();
        let sink = AuditSink::with_file(&path).unwrap();
        sink.push(AuditEvent::new("sys_snapshot", "{}", "read", "ok", "s", 1));
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1, "truncated on open: {content}");
        assert!(lines[0].contains("sys_snapshot"));
    }

    #[test]
    fn file_backend_open_failure_is_loud() {
        let dir = std::env::temp_dir().join(format!("rh-audit-dir-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // 以目录作路径：open+truncate 必败，with_file 必须显式报错
        let err = AuditSink::with_file(&dir).unwrap_err();
        assert!(!err.to_string().is_empty());
    }
}
