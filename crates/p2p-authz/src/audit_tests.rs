//! 审计 sink 测试（authz-a3-plan §1 S2 P1b 验收）：五类事件各一测 + JSONL
//! append-only 语义（重跑不重写旧行）+ 写失败可观测（tracing 日志捕获断言）。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::Layer;

use crate::audit::{append_jsonl, audit_path, record, AuditEvent, AuditKind};
use crate::binding::Binding;
use crate::ops_binding::BoundBinding;
use crate::permissions::Permission;
use crate::role::Role;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2pauthz-audit-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn role(role_id: &str) -> Role {
    Role {
        role_id: role_id.to_owned(),
        name: "测试角色".to_owned(),
        permissions: vec![Permission::parse("chat.send").unwrap()],
        builtin: false,
        note: String::new(),
    }
}

fn binding(peer: &str, role_id: &str) -> Binding {
    Binding {
        peer_id: peer.to_owned(),
        role_id: role_id.to_owned(),
        granted_at: 1_000,
        note: String::new(),
        expires_at: None,
    }
}

fn last_line(dir: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(audit_path(dir)).unwrap();
    let line = text.lines().last().unwrap();
    serde_json::from_str(line).unwrap()
}

fn assert_rfc3339(at: &str) {
    let bytes = at.as_bytes();
    assert_eq!(at.len(), 20, "秒级 RFC3339 形如 2026-02-29T00:00:00Z: {at}");
    assert_eq!(bytes[4], b'-');
    assert_eq!(bytes[7], b'-');
    assert_eq!(bytes[10], b'T');
    assert_eq!(bytes[13], b':');
    assert_eq!(bytes[16], b':');
    assert_eq!(bytes[19], b'Z');
}

#[test]
fn role_created_event_shape() {
    let dir = temp_dir("created");
    record(&dir, &AuditEvent::role_created(&role("tester")));
    let v = last_line(&dir);
    assert_eq!(v["kind"], "authz.role.created");
    assert_eq!(v["actor"], "owner");
    assert_eq!(v["after"]["roleId"], "tester");
    assert_eq!(v["after"]["permissions"], serde_json::json!(["chat.send"]));
    assert_eq!(v["before"], serde_json::Value::Null);
    assert_rfc3339(v["at"].as_str().unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bound_event_shape() {
    let dir = temp_dir("bound");
    record(
        &dir,
        &AuditEvent::bound(&BoundBinding {
            binding: binding("peer-a", "ally"),
            created: true,
        }),
    );
    let v = last_line(&dir);
    assert_eq!(v["kind"], "authz.bound");
    assert_eq!(v["before"], serde_json::Value::Null);
    assert_eq!(v["after"]["peerId"], "peer-a");
    assert_eq!(v["after"]["roleId"], "ally");
    assert_eq!(v["note"], "");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unbound_event_shape() {
    let dir = temp_dir("unbound");
    record(&dir, &AuditEvent::unbound(&binding("peer-a", "ally")));
    let v = last_line(&dir);
    assert_eq!(v["kind"], "authz.unbound");
    assert_eq!(v["before"]["peerId"], "peer-a");
    assert_eq!(v["before"]["roleId"], "ally");
    assert_eq!(v["after"], serde_json::Value::Null);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn role_deleted_event_shape() {
    let dir = temp_dir("deleted");
    record(&dir, &AuditEvent::role_deleted(&role("tester")));
    let v = last_line(&dir);
    assert_eq!(v["kind"], "authz.role.deleted");
    assert_eq!(v["before"]["roleId"], "tester");
    assert_eq!(v["after"], serde_json::Value::Null);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn denied_event_shape() {
    let dir = temp_dir("denied");
    record(
        &dir,
        &AuditEvent::denied("mint-ticket scope=diag peer=p denied: NotBound"),
    );
    let v = last_line(&dir);
    assert_eq!(v["kind"], "authz.denied");
    assert_eq!(v["note"], "mint-ticket scope=diag peer=p denied: NotBound");
    assert_eq!(v["before"], serde_json::Value::Null);
    assert_eq!(v["after"], serde_json::Value::Null);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rebind_upsert_notes_diff() {
    let dir = temp_dir("rebind");
    record(
        &dir,
        &AuditEvent::bound(&BoundBinding {
            binding: binding("peer-a", "friend"),
            created: false,
        }),
    );
    assert_eq!(last_line(&dir)["note"], "upsert", "重绑以 note 区分");
    let _ = std::fs::remove_dir_all(&dir);
}

/// append-only 语义：重跑/幂等重复落账只追加，旧行逐字节不变。
#[test]
fn append_only_preserves_previous_lines() {
    let dir = temp_dir("append");
    record(&dir, &AuditEvent::role_created(&role("tester")));
    let text_v1 = std::fs::read_to_string(audit_path(&dir)).unwrap();
    record(
        &dir,
        &AuditEvent::bound(&BoundBinding {
            binding: binding("peer-a", "tester"),
            created: true,
        }),
    );
    // 重跑同一事件（幂等重放场景）也只追加。
    record(
        &dir,
        &AuditEvent::bound(&BoundBinding {
            binding: binding("peer-a", "tester"),
            created: true,
        }),
    );
    let text_v3 = std::fs::read_to_string(audit_path(&dir)).unwrap();
    assert_eq!(text_v3.lines().count(), 3, "每次落账恰增一行: {text_v3}");
    assert!(
        text_v3.starts_with(&text_v1),
        "旧行逐字节保持不变（append-only 不重写）"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 写失败可观测：append_jsonl 错误上抛（record 的日志信号源），
/// record 本身不 panic 不上抛（不阻塞主操作），且日志留 error 信号。
#[test]
fn write_failure_is_observable_not_silent() {
    let dir = temp_dir("fail");
    std::fs::create_dir_all(&dir).unwrap();
    // data_dir/authz 占位为普通文件：create_dir_all 必败。
    let blocked = dir.join("authz");
    std::fs::write(&blocked, b"not a dir").unwrap();
    let event = AuditEvent::denied("unreachable");
    let err = append_jsonl(&dir, &event).unwrap_err();
    assert!(!err.to_string().is_empty());

    let capture: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let layer = LogCapture(Arc::clone(&capture));
    let subscriber = tracing_subscriber::registry().with(layer);
    tracing::subscriber::with_default(subscriber, || {
        record(&dir, &event); // 不得 panic / 不得上抛
    });
    let logs = capture.lock().unwrap();
    assert!(
        logs.iter().any(|l| l.contains("authz 审计写失败")),
        "写失败必须留 error 日志信号: {logs:?}"
    );
    assert!(
        logs.iter().any(|l| l.contains("authz.denied")),
        "日志须携带 kind 便于定位: {logs:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// tracing 事件捕获层：把字段折叠成一行文本供断言。
struct LogCapture(Arc<Mutex<Vec<String>>>);

impl<S: tracing::Subscriber> Layer<S> for LogCapture {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut collector = FieldCollector(String::new());
        event.record(&mut collector);
        self.0.lock().unwrap().push(collector.0);
    }
}

struct FieldCollector(String);

impl tracing::field::Visit for FieldCollector {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if !self.0.is_empty() {
            self.0.push(' ');
        }
        self.0.push_str(&format!("{}={:?}", field.name(), value));
    }
}

#[test]
fn audit_kind_strings_are_closed_set() {
    assert_eq!(AuditKind::RoleCreated.as_str(), "authz.role.created");
    assert_eq!(AuditKind::Bound.as_str(), "authz.bound");
    assert_eq!(AuditKind::Unbound.as_str(), "authz.unbound");
    assert_eq!(AuditKind::RoleDeleted.as_str(), "authz.role.deleted");
    assert_eq!(AuditKind::Denied.as_str(), "authz.denied");
}
