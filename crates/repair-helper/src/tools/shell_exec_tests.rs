//! shell_exec 执行面测试（T23b）：执法双判/审批/进程执行/输出门禁全链路。
//!
//! 审批时钟用 [ScriptedClock] 注入推进；进程执行均为真实子进程（echo/cat/
//! sleep/touch/rm），全部命令只在本测试夹具内白名单闭集中出现。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use repair_enforce::approval::{ApprovalVerdict, Approver, Clock};
use repair_enforce::scope::Scope;
use repair_enforce::whitelist::{ArgPat, ShellRule, ShellWhitelist};
use serde_json::json;

use super::approval::{QueueApprover, ScriptedClock};
use super::shell_exec::ShellExec;
use crate::cap::MAX_OUTPUT_BYTES;
use crate::enforce::Enforcement;
use crate::jail::PathJail;
use crate::Tool;

fn fixture(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("rh-shell-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn jailed(root: PathBuf) -> PathJail {
    PathJail::from_roots(vec![root]).unwrap()
}

fn whitelist(prog: &str, args: Vec<ArgPat>) -> ShellWhitelist {
    let mut w = ShellWhitelist::empty();
    w.add(ShellRule::new(prog, args));
    w
}

/// 可外部喂裁决的审批通道（保留 QueueApprover 句柄供 push）。
fn approver_with_queue() -> (Arc<Mutex<Box<dyn Approver + Send>>>, QueueApprover) {
    let q = QueueApprover::new();
    (
        Arc::new(Mutex::new(Box::new(q.clone()) as Box<dyn Approver + Send>)),
        q,
    )
}

fn empty_approver() -> Arc<Mutex<Box<dyn Approver + Send>>> {
    Arc::new(Mutex::new(
        Box::new(QueueApprover::new()) as Box<dyn Approver + Send>
    ))
}

fn scripted_clock() -> Arc<dyn Clock + Send + Sync> {
    Arc::new(ScriptedClock::new(Duration::ZERO))
}

fn shell(
    jail: PathJail,
    scope: Scope,
    wl: ShellWhitelist,
    approver: Arc<Mutex<Box<dyn Approver + Send>>>,
) -> ShellExec {
    ShellExec::new(
        jail,
        Enforcement::new(scope, wl),
        scripted_clock(),
        approver,
    )
}

#[tokio::test]
async fn whitelist_hit_approved_executes_with_output() {
    let root = fixture("ok");
    let (appr, q) = approver_with_queue();
    q.push(ApprovalVerdict::Approved);
    let tool = shell(
        jailed(root),
        Scope::Fix,
        whitelist("echo", vec![ArgPat::Any]),
        appr,
    );
    let result = tool.call(json!({"argv": ["echo", "hi"]})).await.unwrap();
    assert!(result.text.contains("exit=0"), "{}", result.text);
    assert!(result.text.contains("killed=none"), "{}", result.text);
    assert!(result.text.contains("hi"), "{}", result.text);
    assert!(!result.truncated);
}

#[tokio::test]
async fn closed_set_rejected_and_not_spawned() {
    let root = fixture("nospawn");
    let (appr, _) = approver_with_queue();
    let tool = shell(
        jailed(root.clone()),
        Scope::Fix,
        whitelist("echo", vec![]),
        appr,
    );
    let err = tool
        .call(json!({"argv": ["touch", "boom.txt"]}))
        .await
        .unwrap_err();
    assert!(err.contains("not in closed whitelist"), "{}", err);
    assert!(!root.join("boom.txt").exists());
}

#[tokio::test]
async fn argument_pattern_mismatch_rejected_and_not_spawned() {
    let root = fixture("mismatch");
    let (appr, _) = approver_with_queue();
    let tool = shell(
        jailed(root.clone()),
        Scope::Fix,
        whitelist("touch", vec![ArgPat::exact("KEY")]),
        appr,
    );
    let err = tool
        .call(json!({"argv": ["touch", "other"]}))
        .await
        .unwrap_err();
    assert!(err.contains("argument pattern"), "{}", err);
    assert!(!root.join("other").exists());
}

#[tokio::test]
async fn timeout_kills_and_reports() {
    let root = fixture("timeout");
    let (appr, q) = approver_with_queue();
    q.push(ApprovalVerdict::Approved);
    let tool = shell(
        jailed(root),
        Scope::Fix,
        whitelist("sleep", vec![ArgPat::Any]),
        appr,
    );
    let started = std::time::Instant::now();
    let result = tool
        .call(json!({"argv": ["sleep", "30"], "timeout": 1}))
        .await
        .unwrap();
    assert!(result.text.contains("killed=timeout"), "{}", result.text);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "kill took too long"
    );
}

#[tokio::test]
async fn huge_output_truncated_flagged() {
    let root = fixture("trunc");
    std::fs::write(root.join("big.txt"), "a".repeat(MAX_OUTPUT_BYTES + 4096)).unwrap();
    let (appr, q) = approver_with_queue();
    q.push(ApprovalVerdict::Approved);
    let tool = shell(
        jailed(root),
        Scope::Fix,
        whitelist("cat", vec![ArgPat::Any]),
        appr,
    );
    let result = tool
        .call(json!({"argv": ["cat", "big.txt"], "timeout": 20}))
        .await
        .unwrap();
    assert!(result.truncated);
    assert!(result.text.len() <= MAX_OUTPUT_BYTES);
    assert!(result.text.contains('a'), "{}", result.text);
}

#[tokio::test]
async fn redline_wins_over_whitelist() {
    let root = fixture("redline");
    let (appr, _) = approver_with_queue();
    let tool = shell(
        jailed(root),
        Scope::Fix,
        whitelist("rm", vec![ArgPat::prefix("-"), ArgPat::Any, ArgPat::Any]),
        appr,
    );
    let err = tool
        .call(json!({"argv": ["rm", "-rf", "/"]}))
        .await
        .unwrap_err();
    assert!(err.contains("redline"), "{}", err);
}

#[tokio::test]
async fn diag_scope_denied_before_approval() {
    let root = fixture("diag");
    let (appr, _) = approver_with_queue();
    let tool = shell(
        jailed(root),
        Scope::Diag,
        whitelist("echo", vec![ArgPat::Any]),
        appr,
    );
    let err = tool
        .call(json!({"argv": ["echo", "hi"]}))
        .await
        .unwrap_err();
    assert!(err.contains("diag"), "{}", err);
}

#[tokio::test]
async fn approval_denied_does_not_spawn() {
    let root = fixture("apprdeny");
    let (appr, q) = approver_with_queue();
    q.push(ApprovalVerdict::Denied);
    let tool = shell(
        jailed(root.clone()),
        Scope::Fix,
        whitelist("touch", vec![ArgPat::Any]),
        appr,
    );
    let err = tool
        .call(json!({"argv": ["touch", "m.txt"]}))
        .await
        .unwrap_err();
    assert!(err.contains("approval denied"), "{}", err);
    assert!(!root.join("m.txt").exists());
}

#[tokio::test]
async fn approval_timeout_is_denial() {
    let root = fixture("apprtimeout");
    let scripted = Arc::new(ScriptedClock::new(Duration::ZERO));
    let clk: Arc<dyn Clock + Send + Sync> = scripted.clone();
    let tool = ShellExec::new(
        jailed(root),
        Enforcement::new(Scope::Fix, whitelist("echo", vec![ArgPat::Any])),
        clk,
        empty_approver(),
    );
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        scripted.advance(Duration::from_secs(61));
    });
    let err = tool
        .call(json!({"argv": ["echo", "hi"]}))
        .await
        .unwrap_err();
    assert!(err.contains("approval timed out"), "{}", err);
}

#[tokio::test]
async fn cwd_escape_rejected_before_spawn() {
    let root = fixture("cwdesc");
    let (appr, _) = approver_with_queue();
    let tool = shell(
        jailed(root),
        Scope::Fix,
        whitelist("cat", vec![ArgPat::Any]),
        appr,
    );
    let err = tool
        .call(json!({"argv": ["cat", "x.txt"], "cwd": "../escape"}))
        .await
        .unwrap_err();
    assert!(err.contains("cwd"), "{}", err);
}
