//! in-process pump 契约用例：真实 Pump::start 回环（临时目录，随机端口）、
//! 停机收尾迁移、退出原因→状态面映射（含异常失败态）、命令 serde roundtrip、
//! 事件载荷盖戳。无外部进程 stub（sidecar 机制已删）。

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tokio::sync::watch;

use super::event_payload;
use super::types::{AcpConsolePhase, AcpConsoleStatus};

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("ux_console_inproc_{tag}_{}", std::process::id()));
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

fn subscribe_of(manager: &super::Manager) -> watch::Receiver<AcpConsoleStatus> {
    manager.subscribe()
}

#[tokio::test]
async fn in_process_pump_connects_then_disconnected_on_shutdown() {
    let dir = TempDir::new("lifecycle");
    let manager = super::Manager::spawn(dir.0.clone());
    let mut rx = subscribe_of(&manager);
    let status = wait_for(&mut rx, "connected", |s| {
        s.phase == AcpConsolePhase::Connected
    })
    .await;
    let ws_url = status.ws_url.as_deref().expect("connected 必须携带 wsUrl");
    assert!(ws_url.starts_with("ws://127.0.0.1:"), "wsUrl={ws_url}");
    assert!(
        status
            .token
            .as_deref()
            .map(|t| !t.is_empty())
            .unwrap_or(false),
        "connected 必须携带 token"
    );
    assert!(
        status
            .status_url
            .as_deref()
            .map(|u| u.starts_with("http://127.0.0.1:"))
            .unwrap_or(false),
        "statusUrl={:?}",
        status.status_url
    );
    assert!(
        dir.0.join("acp-console-data").is_dir(),
        "数据目录必须仍是 app_data_dir/acp-console-data"
    );

    manager.shutdown();
    manager.shutdown();
    let down = wait_for(&mut rx, "disconnected", |s| {
        s.phase == AcpConsolePhase::Disconnected
    })
    .await;
    assert!(
        down.ws_url.is_none() && down.token.is_none(),
        "收尾后连接面清空"
    );
    assert!(down.last_error.is_none(), "干净收尾不留错误: {down:?}");
}

#[test]
fn abnormal_exit_maps_to_disconnected_with_last_error() {
    let failed = acp_pump::PumpExit {
        ok: false,
        error: Some("panic: boom".to_string()),
    };
    let status = super::exit_to_status(&failed);
    assert_eq!(status.phase, AcpConsolePhase::Disconnected);
    assert_eq!(status.last_error.as_deref(), Some("panic: boom"));
    assert!(
        status.ws_url.is_none() && status.token.is_none(),
        "失败态连接面清空"
    );

    let clean = acp_pump::PumpExit {
        ok: true,
        error: None,
    };
    let status = super::exit_to_status(&clean);
    assert_eq!(status.phase, AcpConsolePhase::Disconnected);
    assert!(status.last_error.is_none(), "干净收尾无错误");
}

#[test]
fn status_serde_roundtrip_contract() {
    let full = AcpConsoleStatus {
        phase: AcpConsolePhase::Connected,
        ws_url: Some("ws://127.0.0.1:9101".into()),
        token: Some("t".into()),
        status_url: Some("http://127.0.0.1:9102".into()),
        last_error: Some("earlier".into()),
    };
    crate::types::testing::roundtrip(
        &full,
        serde_json::json!({
            "phase": "connected",
            "wsUrl": "ws://127.0.0.1:9101",
            "token": "t",
            "statusUrl": "http://127.0.0.1:9102",
            "lastError": "earlier",
        }),
    );
    let minimal: AcpConsoleStatus =
        serde_json::from_value(serde_json::json!({ "phase": "disconnected" }))
            .expect("缺省字段必须容忍");
    assert_eq!(minimal.phase, AcpConsolePhase::Disconnected);
    assert!(minimal.ws_url.is_none() && minimal.token.is_none());
}

#[test]
fn event_payload_stamps_ts_ms() {
    let payload = event_payload(&AcpConsoleStatus::disconnected(Some(
        "装配失败".to_string(),
    )))
    .expect("载荷");
    assert_eq!(payload["phase"], "disconnected");
    assert_eq!(payload["lastError"], "装配失败");
    assert!(
        payload["tsMs"].is_u64(),
        "事件载荷须带 tsMs 盖戳: {payload}"
    );
}
