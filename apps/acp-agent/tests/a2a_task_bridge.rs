//! task⇄ACP 桥行为测试（直驱 TaskService，无 p2p）：§9 权限矩阵的桥内闭环——
//! public agent execute 一律 OwnerLocal 立即拒绝占位；local agent execute 走
//! RemoteGui 路由，无应答通道时超时代答 reject-once（owner 不在场即拒绝）。
//! 子进程用 acp-echo-stub --acp-perm 模式（CARGO_BIN_EXE 定位）。

use std::sync::Arc;
use std::time::Duration;

use a2a::{Message, Part, TaskState};
use acp_agent::a2a::bridge::BridgeEvent;
use acp_agent::a2a::TaskService;
use acp_agent::{AuditEvent, CaptureAudit};

/// 单步等待上限：桥握手 + 权限超时（1s）+ 宽限，5s 为宽松护栏。
const STEP: Duration = Duration::from_secs(5);

fn service_with(tag: &str, visibility: a2a::Visibility, stub_args: &[&str]) -> (Arc<TaskService>, Arc<CaptureAudit>, String) {
    let dir = std::env::temp_dir().join(format!("a2a-bridge-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("tmp dir");
    let mut command = vec![env!("CARGO_BIN_EXE_acp-echo-stub").to_owned()];
    command.extend(stub_args.iter().map(|s| s.to_string()));
    let cfg = acp_agent::AgentConfig {
        data_dir: dir.to_string_lossy().into_owned(),
        command,
        grace_secs: 1,
        permission_timeout_secs: 1,
        descriptor_disabled: true,
        admin_disabled: true,
        ..acp_agent::AgentConfig::default()
    };
    let agents = acp_agent::a2a::AgentStore::open(dir.join("agents.json")).expect("agents");
    agents
        .create(
            Some("agent-1".into()),
            "测试".into(),
            "d".into(),
            vec![],
            visibility,
            1,
        )
        .expect("create agent");
    let grants = acp_agent::a2a::GrantStore::open(dir.join("grants.json")).expect("grants");
    let audit = Arc::new(CaptureAudit::new());
    let ws = Arc::new(
        acp_agent::workspaces::WorkspaceStore::open(&[], None, cfg.paths().workspaces())
            .expect("ws"),
    );
    let service = Arc::new(TaskService::new(
        cfg,
        agents,
        Arc::new(grants),
        ws,
        audit.clone(),
    ));
    (service, audit, dir.to_string_lossy().into_owned())
}

async fn run_task(
    service: &Arc<TaskService>,
    is_owner: bool,
) -> (Vec<String>, TaskState) {
    let handle = service
        .create("peer-a", is_owner, "agent-1", Message::user_text("跑一轮"))
        .await
        .expect("create task");
    let mut rx = handle.take_events().expect("events");
    let mut texts = Vec::new();
    let state = loop {
        match tokio::time::timeout(STEP, rx.recv())
            .await
            .expect("event wait timeout")
            .expect("event channel closed")
        {
            BridgeEvent::Message { part: Part::Text(t), .. } => texts.push(t.text),
            BridgeEvent::Message { .. } => {}
            BridgeEvent::Done(state) => break state,
        }
    };
    (texts, state)
}

#[tokio::test]
async fn public_agent_execute_is_owner_local_rejected() {
    let (service, audit, dir) = service_with(
        "pub-exec",
        a2a::Visibility::Public,
        &["--acp-agent", "--acp-perm", "execute"],
    );
    let (texts, state) = run_task(&service, false).await;
    assert_eq!(state, TaskState::Completed);
    assert!(
        texts.iter().any(|t| t.contains("perm:cancelled")),
        "execute 必须被 OwnerLocal 拒绝后继续: {texts:?}"
    );
    assert!(
        audit.contains(|e| matches!(e, AuditEvent::PermissionActed { action, .. } if action == "owner-local")),
        "OwnerLocal 处置必须留审计"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// §9 local 行 + Q3 超时拍板：owner 全权 agent 的 execute 走 RemoteGui 路由，
/// 无 GUI 应答通道时 1s 超时 reject-once，任务照常结算。
#[tokio::test]
async fn local_agent_execute_ask_times_out_to_reject_once() {
    let (service, audit, dir) = service_with(
        "local-exec",
        a2a::Visibility::Local,
        &["--acp-agent", "--acp-perm", "execute"],
    );
    let started = std::time::Instant::now();
    let (texts, state) = run_task(&service, true).await;
    assert_eq!(state, TaskState::Completed);
    assert!(
        texts.iter().any(|t| t.contains("perm:cancelled")),
        "超时必须代答 reject-once: {texts:?}"
    );
    assert!(
        started.elapsed() >= Duration::from_millis(900),
        "拒绝必须发生在超时窗之后（真实等待）"
    );
    assert!(
        audit.contains(|e| matches!(e, AuditEvent::PermissionActed { action, .. } if action == "timeout-rejected")),
        "超时代答必须留审计"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// local agent read 静态放行（§9 local 行 read/fetch 列）——对照 execute 的 ask。
#[tokio::test]
async fn local_agent_read_is_static_allowed() {
    let (service, audit, dir) = service_with(
        "local-read",
        a2a::Visibility::Local,
        &["--acp-agent", "--acp-perm", "read"],
    );
    let (texts, state) = run_task(&service, true).await;
    assert_eq!(state, TaskState::Completed);
    assert!(
        !texts.iter().any(|t| t.contains("perm:cancelled")),
        "read 必须静态放行不落拒绝: {texts:?}"
    );
    assert!(
        audit.contains(|e| matches!(e, AuditEvent::PermissionActed { action, .. } if action == "auto-allowed")),
        "静态放行必须留审计"
    );
    let _ = std::fs::remove_dir_all(dir);
}
