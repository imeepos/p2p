//! 契约 §15 验收用例（stub 子进程）：ready 解析、重启退避、failed 迁移、Exit 收尾、
//! 命令 serde roundtrip、事件载荷盖戳。stub 为 /bin/sh 脚本，仅 unix（本机与 CI 均 unix）。
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tokio::sync::watch;

use super::event_payload;
use super::handle::{launch, SupervisorHandle};
use super::spec::{backoff_for, Limits, SpawnSpec};
use super::status::{AcpConsolePhase, AcpConsoleStatus};

/// 就绪后常驻（sleep 30s，测试经 shutdown 收敛）。
const STUB_READY_BODY: &str = r#"#!/bin/sh
printf '%s\n' '{"kind":"ready","ws":"127.0.0.1:9101","status":"127.0.0.1:9102","token":"tok-1","peer":"self-peer"}'
sleep 30
"#;

/// 立即崩溃退出。
const STUB_CRASH_BODY: &str = r#"#!/bin/sh
exit 1
"#;

/// 持续输出非法行（解析失败计失败观测，第 5 次达上限）。
const STUB_GARBAGE_BODY: &str = r#"#!/bin/sh
i=0
while [ "$i" -lt 10 ]; do
  printf '%s\n' 'not-json'
  i=$((i + 1))
  sleep 0.05
done
sleep 30
"#;

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("ux_console_t_{tag}_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("创建临时目录");
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            eprintln!("[console-test] 清理临时目录失败 {}: {e}", self.0.display());
        }
    }
}

fn write_stub(dir: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("acp-console");
    std::fs::write(&path, body).expect("写 stub 脚本");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("stub 加执行位");
    path
}

fn fast_limits(max: u32) -> Limits {
    Limits {
        max_consecutive_failures: max,
        backoff_base: Duration::from_millis(5),
        backoff_cap: Duration::from_millis(30),
    }
}

fn drive(stub: &Path, limits: Limits) -> (SupervisorHandle, watch::Receiver<AcpConsoleStatus>) {
    let (tx, rx) = watch::channel(AcpConsoleStatus::initial());
    let spec = SpawnSpec {
        bin: stub.to_path_buf(),
        args: Vec::new(),
    };
    let (handle, fut) = launch(spec, limits, tx);
    tokio::spawn(fut);
    (handle, rx)
}

/// 轮询状态直至满足条件；超时 panic 并携带最后快照（失败路径显式可观测）。
async fn wait_for(
    rx: &mut watch::Receiver<AcpConsoleStatus>,
    label: &str,
    pred: impl Fn(&AcpConsoleStatus) -> bool,
) -> AcpConsoleStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let status = rx.borrow().clone();
        if pred(&status) {
            return status;
        }
        if Instant::now() >= deadline {
            panic!("等待 {label} 超时，最后状态: {status:?}");
        }
        let _ = tokio::time::timeout(Duration::from_millis(200), rx.changed()).await;
    }
}

#[tokio::test]
async fn ready_stub_assembles_contract_face() {
    let dir = TempDir::new("ready");
    let stub = write_stub(&dir.0, STUB_READY_BODY);
    let (_handle, mut rx) = drive(&stub, fast_limits(5));
    let status = wait_for(&mut rx, "ready", |s| s.phase == AcpConsolePhase::Ready).await;
    assert_eq!(status.ws_url.as_deref(), Some("ws://127.0.0.1:9101"));
    assert_eq!(status.token.as_deref(), Some("tok-1"));
    assert_eq!(status.status_url.as_deref(), Some("http://127.0.0.1:9102"));
    assert_eq!(status.admin_url, None, "ready 行未携带 admin 时容忍缺省");
    assert_eq!(status.restarts, 0);
}

#[tokio::test]
async fn crash_stub_restarts_with_backoff() {
    let dir = TempDir::new("crash");
    let stub = write_stub(&dir.0, STUB_CRASH_BODY);
    let (_handle, mut rx) = drive(&stub, fast_limits(5));
    let status = wait_for(&mut rx, "restarts>=2", |s| s.restarts >= 2).await;
    assert!(status.restarts >= 2, "崩溃后须自动重启: {status:?}");
    let err = status.last_error.clone().expect("重启须留 lastError");
    assert!(err.contains("退出"), "崩溃原因应含退出描述: {err}");
    assert!(status.ws_url.is_none(), "崩溃后连接面必须清空");
    assert!(
        matches!(
            status.phase,
            AcpConsolePhase::Restarting | AcpConsolePhase::Starting
        ),
        "失败后处于重启链路: {status:?}"
    );
}

#[tokio::test]
async fn consecutive_failures_flip_failed_and_stop_retrying() {
    let dir = TempDir::new("failed");
    let stub = write_stub(&dir.0, STUB_CRASH_BODY);
    let (_handle, mut rx) = drive(&stub, fast_limits(2));
    let status = wait_for(&mut rx, "failed", |s| s.phase == AcpConsolePhase::Failed).await;
    assert_eq!(
        status.restarts, 1,
        "连续失败 2 次即转 failed，只允许 1 次重启"
    );
    assert!(status.last_error.is_some());
    let frozen = status.restarts;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let after = rx.borrow().clone();
    assert_eq!(after.phase, AcpConsolePhase::Failed, "failed 为终态");
    assert_eq!(after.restarts, frozen, "failed 后必须停止重启");
}

#[tokio::test]
async fn garbage_lines_count_toward_failed() {
    let dir = TempDir::new("garbage");
    let stub = write_stub(&dir.0, STUB_GARBAGE_BODY);
    let (_handle, mut rx) = drive(&stub, fast_limits(5));
    let status = wait_for(&mut rx, "解析失败转 failed", |s| {
        s.phase == AcpConsolePhase::Failed
    })
    .await;
    let err = status.last_error.expect("解析失败须留痕");
    assert!(err.contains("非法 JSON"), "留痕须指明非法行: {err}");
}

#[tokio::test]
async fn shutdown_kills_child_and_marks_stopped() {
    let dir = TempDir::new("exit");
    let stub = write_stub(&dir.0, STUB_READY_BODY);
    let (handle, mut rx) = drive(&stub, fast_limits(5));
    wait_for(&mut rx, "ready", |s| s.phase == AcpConsolePhase::Ready).await;
    let pid = handle.current_pid().expect("ready 后应有子进程 pid");
    handle.shutdown();
    handle.shutdown();
    let status = wait_for(&mut rx, "stopped", |s| s.phase == AcpConsolePhase::Stopped).await;
    assert!(status.ws_url.is_none(), "收尾后连接面清空");
    assert!(!pid_alive(pid), "子进程 {pid} 必须已被终止");
}

/// kill -0 探活：成功 = 进程仍在。
fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(true)
}

#[test]
fn status_serde_roundtrip_contract() {
    let full = AcpConsoleStatus {
        phase: AcpConsolePhase::Restarting,
        ws_url: None,
        token: Some("t".into()),
        status_url: Some("http://127.0.0.1:1".into()),
        admin_url: None,
        restarts: 3,
        last_error: Some("acp-console 进程退出（exit status: 1）".into()),
    };
    crate::types::testing::roundtrip(
        &full,
        serde_json::json!({
            "phase": "restarting",
            "wsUrl": null,
            "token": "t",
            "statusUrl": "http://127.0.0.1:1",
            "adminUrl": null,
            "restarts": 3,
            "lastError": "acp-console 进程退出（exit status: 1）",
        }),
    );
    let minimal: AcpConsoleStatus =
        serde_json::from_value(serde_json::json!({ "phase": "ready", "restarts": 0 }))
            .expect("缺省字段必须容忍");
    assert_eq!(minimal.phase, AcpConsolePhase::Ready);
    assert!(minimal.ws_url.is_none() && minimal.token.is_none());
}

#[test]
fn event_payload_stamps_ts_ms() {
    let payload = event_payload(&AcpConsoleStatus::unavailable("定位失败")).expect("载荷");
    assert_eq!(payload["phase"], "unavailable");
    assert_eq!(payload["lastError"], "定位失败");
    assert!(
        payload["tsMs"].is_u64(),
        "事件载荷须带 tsMs 盖戳: {payload}"
    );
}

#[test]
fn backoff_grows_exponentially_and_caps() {
    let limits = Limits {
        max_consecutive_failures: 9,
        backoff_base: Duration::from_millis(100),
        backoff_cap: Duration::from_millis(800),
    };
    assert_eq!(backoff_for(&limits, 1), Duration::from_millis(100));
    assert_eq!(backoff_for(&limits, 2), Duration::from_millis(200));
    assert_eq!(backoff_for(&limits, 3), Duration::from_millis(400));
    assert_eq!(backoff_for(&limits, 4), Duration::from_millis(800));
    assert_eq!(backoff_for(&limits, 9), Duration::from_millis(800));
}
