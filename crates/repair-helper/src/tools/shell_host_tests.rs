//! 宿主级 shell_exec 接线测试：guarded Host 的执法门 + 审批放行 + 审计事件。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use repair_enforce::approval::{ApprovalVerdict, Approver, Clock};
use repair_enforce::scope::Scope;
use repair_enforce::whitelist::{ArgPat, ShellRule, ShellWhitelist};
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::watch;

use super::approval::{QueueApprover, ScriptedClock};
use super::shell_exec::ShellExec;
use crate::audit::AuditSink;
use crate::enforce::Enforcement;
use crate::jail::PathJail;
use crate::tools;
use crate::Host;

fn fixture(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("rh-shell-host-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn whitelist(prog: &str, args: Vec<ArgPat>) -> ShellWhitelist {
    let mut w = ShellWhitelist::empty();
    w.add(ShellRule::new(prog, args));
    w
}

fn approver_approved() -> Arc<Mutex<Box<dyn Approver + Send>>> {
    let q = QueueApprover::new();
    q.push(ApprovalVerdict::Approved);
    Arc::new(Mutex::new(Box::new(q) as Box<dyn Approver + Send>))
}

fn clock() -> Arc<dyn Clock + Send + Sync> {
    Arc::new(ScriptedClock::new(Duration::ZERO))
}

fn host_with_shell(root: PathBuf, scope: Scope, wl: ShellWhitelist) -> (Host, AuditSink) {
    let jail = PathJail::from_roots(vec![root]).unwrap();
    let shell = ShellExec::new(
        jail.clone(),
        Enforcement::new(scope, wl.clone()),
        clock(),
        approver_approved(),
    );
    let audit = AuditSink::default();
    let registry = tools::helper_registry(jail, shell, audit.clone());
    (
        Host::guarded(registry, Enforcement::new(scope, wl), audit.clone()),
        audit,
    )
}

async fn run_host(host: Host, input: &str) -> String {
    let (mut client, server) = duplex(1 << 20);
    let (reader, writer) = tokio::io::split(server);
    let (_tx, rx) = watch::channel(false);
    let task = tokio::spawn(host.serve(BufReader::new(reader), writer, rx));
    client.write_all(input.as_bytes()).await.unwrap();
    client.shutdown().await.unwrap();
    let mut output = String::new();
    BufReader::new(client)
        .read_to_string(&mut output)
        .await
        .unwrap();
    let _ = task.await;
    output
}

#[tokio::test]
async fn host_fix_approved_executes_and_audits() {
    let root = fixture("fixok");
    let (host, audit) = host_with_shell(root, Scope::Fix, whitelist("echo", vec![ArgPat::Any]));
    let output = run_host(
        host,
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"shell_exec\",\"arguments\":{\"argv\":[\"echo\",\"hi\"]}}}\n",
    )
    .await;
    assert!(output.contains("exit=0"), "{}", output);
    assert!(output.contains("\"isError\":false"), "{}", output);
    let events = audit.snapshot();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].tool, "shell_exec");
    assert_eq!(events[0].risk, "write");
    assert_eq!(events[0].outcome, "ok");
    assert!(
        events[0].result_summary.contains("exit=0"),
        "{}",
        events[0].result_summary
    );
}

#[tokio::test]
async fn host_diag_denied_and_audited() {
    let root = fixture("diagdeny");
    let (host, audit) = host_with_shell(root, Scope::Diag, whitelist("echo", vec![ArgPat::Any]));
    let output = run_host(
        host,
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"shell_exec\",\"arguments\":{\"argv\":[\"echo\",\"hi\"]}}}\n",
    )
    .await;
    assert!(output.contains("denied"), "{}", output);
    let events = audit.snapshot();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].outcome, "denied");
    assert_eq!(events[0].risk, "write");
}
