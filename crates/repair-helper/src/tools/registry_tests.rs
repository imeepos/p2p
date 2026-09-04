//! helper_registry 装配形状测试：六工具闭集 + session_report 共享审计源。

use std::time::Duration;

use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::watch;

use super::*;
use crate::audit::AuditEvent;
use crate::enforce::Enforcement;
use crate::jail::PathJail;
use crate::tools::approval::{QueueApprover, ScriptedClock};
use crate::Host;

fn fixture(tag: &str) -> PathJail {
    let root = std::env::temp_dir().join(format!("rh-reg-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    PathJail::from_roots(vec![root]).unwrap()
}

fn approver() -> std::sync::Arc<std::sync::Mutex<Box<dyn repair_enforce::approval::Approver + Send>>>
{
    std::sync::Arc::new(std::sync::Mutex::new(Box::new(QueueApprover::new())))
}

fn clock() -> std::sync::Arc<dyn repair_enforce::approval::Clock + Send + Sync> {
    std::sync::Arc::new(ScriptedClock::new(Duration::ZERO))
}

fn host_with_registry(tag: &str) -> (Host, AuditSink) {
    let jail = fixture(tag);
    let enforcement = Enforcement::new(repair_enforce::Scope::Diag, repair_enforce::builtin());
    let shell = shell_exec::ShellExec::new(jail.clone(), enforcement.clone(), clock(), approver());
    let audit = AuditSink::default();
    let registry = helper_registry(jail, shell, audit.clone());
    (Host::guarded(registry, enforcement, audit.clone()), audit)
}

#[tokio::test]
async fn helper_registry_exposes_six_tool_closed_set() {
    let (host, _audit) = host_with_registry("shape");
    let (mut client, server) = duplex(1 << 20);
    let (reader, writer) = tokio::io::split(server);
    let (_tx, rx) = watch::channel(false);
    let task = tokio::spawn(host.serve(BufReader::new(reader), writer, rx));
    client
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\",\"params\":{}}\n")
        .await
        .unwrap();
    client.shutdown().await.unwrap();
    let mut output = String::new();
    BufReader::new(client)
        .read_to_string(&mut output)
        .await
        .unwrap();
    let _ = task.await;
    let line = output.lines().next().expect("one response line");
    let reply: serde_json::Value = serde_json::from_str(line).unwrap();
    let mut names: Vec<String> = reply["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "fs_list",
            "fs_read",
            "fs_search",
            "session_report",
            "shell_exec",
            "sys_snapshot"
        ],
        "{names:?}"
    );
}

#[tokio::test]
async fn session_report_exports_shared_audit_events() {
    let (host, audit) = host_with_registry("share");
    audit.push(AuditEvent::new("fs_read", "{}", "read", "ok", "hi", 1));
    let (mut client, server) = duplex(1 << 20);
    let (reader, writer) = tokio::io::split(server);
    let (_tx, rx) = watch::channel(false);
    let task = tokio::spawn(host.serve(BufReader::new(reader), writer, rx));
    client
        .write_all(
            b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"session_report\",\"arguments\":{}}}\n",
        )
        .await
        .unwrap();
    client.shutdown().await.unwrap();
    let mut output = String::new();
    BufReader::new(client)
        .read_to_string(&mut output)
        .await
        .unwrap();
    let _ = task.await;
    let line = output.lines().next().expect("one response line");
    let reply: serde_json::Value = serde_json::from_str(line).unwrap();
    let text = reply["result"]["content"][0]["text"].as_str().unwrap();
    let report: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(report["ticketId"], STDIO_TICKET_ID);
    assert_eq!(report["count"], 1);
    assert_eq!(report["events"][0]["tool"], "fs_read");
}
