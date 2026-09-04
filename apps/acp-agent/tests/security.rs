//! ACP4 安全层回环集成测试（cwd 监狱 + mcpServers）：sandbox 每 peer 监狱、
//! workspace 锁定授权目录、未配置拒绝；默认剥离、白名单按名替换、白名单外拒绝。
//! 全部单机双 facade 节点 + 回声桩：客户端注入的行经桩回声后等价于子进程自发。

mod common;

use acp_agent::AuditEvent;
use acp_common::{AskRoute, Scope, ServerHello};
use common::{
    connect_and_stream, handshake_client, read_line, rig, send_line, session_new_with_mcp,
    shutdown, test_grant_full,
};
use serde_json::Value;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sandbox_scope_jails_cwd_per_peer() {
    let grant = test_grant_full(Scope::Sandbox, Vec::new(), AskRoute::RemoteGui);
    let rig = rig("jail", grant, |cfg| {
        cfg.command = vec![common::STUB.to_owned(), "--print-cwd".to_owned()];
    })
    .await;
    let mut stream = connect_and_stream(&rig.client, rig.server_peer).await;
    handshake_client(&mut stream).await;
    let line = read_line(&mut stream).await.expect("cwd line");
    // 监狱按远程 peer（客户端）命名，而非桥自身
    let expected = rig.client.local_peer_id().to_string();
    assert!(
        line.contains(&expected),
        "cwd must live inside the per-peer jail: {line} vs peer {expected}",
    );
    assert!(
        line.contains("sandbox"),
        "jail must live under sandbox root: {line}",
    );
    shutdown(&rig);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_scope_locks_configured_dir() {
    let grant = test_grant_full(Scope::Workspace, Vec::new(), AskRoute::RemoteGui);
    let ws = std::env::temp_dir().join(format!("acp-ws-it-{}", std::process::id()));
    std::fs::create_dir_all(&ws).expect("workspace dir");
    let ws_path = ws.clone();
    let rig = rig("ws", grant, move |cfg| {
        cfg.command = vec![common::STUB.to_owned(), "--print-cwd".to_owned()];
        cfg.workspace_dir = Some(ws_path.to_string_lossy().into_owned());
    })
    .await;
    let mut stream = connect_and_stream(&rig.client, rig.server_peer).await;
    handshake_client(&mut stream).await;
    let line = read_line(&mut stream).await.expect("cwd line");
    assert_eq!(
        std::path::Path::new(line.trim_end()),
        ws.canonicalize().expect("canonical ws").as_path(),
        "workspace scope must lock the configured dir, got {line}",
    );
    shutdown(&rig);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_without_configured_dir_is_denied() {
    let grant = test_grant_full(Scope::Workspace, Vec::new(), AskRoute::RemoteGui);
    let rig = rig("ws-deny", grant, |_| {}).await;
    let mut stream = connect_and_stream(&rig.client, rig.server_peer).await;
    let reply = handshake_client(&mut stream).await;
    assert_eq!(
        reply,
        ServerHello::Denied {
            denied: "cwd-denied".to_owned(),
        },
        "workspace scope without dir must be denied on the wire",
    );
    assert!(
        rig.audit
            .contains(|ev| matches!(ev, AuditEvent::CwdDenied { .. })),
        "cwd denial must be audited: {:?}",
        rig.audit.snapshot(),
    );
    shutdown(&rig);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_servers_stripped_by_default() {
    let grant = test_grant_full(Scope::Sandbox, Vec::new(), AskRoute::RemoteGui);
    let rig = rig("mcp-strip", grant, |_| {}).await;
    let mut stream = connect_and_stream(&rig.client, rig.server_peer).await;
    handshake_client(&mut stream).await;
    let line = session_new_with_mcp(
        1,
        serde_json::json!([{ "command": "evil", "args": ["-x"] }]),
    );
    send_line(&mut stream, &line).await;
    let echoed = read_line(&mut stream).await.expect("echo");
    let root: Value = serde_json::from_str(echoed.trim_end()).expect("json");
    assert!(
        root["params"].get("mcpServers").is_none(),
        "mcpServers must be stripped by default: {echoed}",
    );
    assert!(
        rig.audit.contains(|ev| matches!(
            ev,
            AuditEvent::McpRewritten { action, .. } if action == "stripped"
        )),
        "strip must be audited: {:?}",
        rig.audit.snapshot(),
    );
    shutdown(&rig);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_whitelist_replaces_with_host_definitions() {
    let grant = test_grant_full(Scope::Sandbox, vec!["fs".to_owned()], AskRoute::RemoteGui);
    let rig = rig("mcp-repl", grant, |cfg| {
        cfg.mcp_definitions.insert(
            "fs".to_owned(),
            serde_json::json!({ "command": "node", "args": ["fs-server.js"] }),
        );
    })
    .await;
    let mut stream = connect_and_stream(&rig.client, rig.server_peer).await;
    handshake_client(&mut stream).await;
    let line = session_new_with_mcp(2, serde_json::json!([{ "name": "fs" }]));
    send_line(&mut stream, &line).await;
    let echoed = read_line(&mut stream).await.expect("echo");
    let root: Value = serde_json::from_str(echoed.trim_end()).expect("json");
    assert_eq!(
        root["params"]["mcpServers"][0]["command"], "node",
        "entry must be replaced by the host definition: {echoed}",
    );
    assert!(
        rig.audit.contains(|ev| matches!(
            ev,
            AuditEvent::McpRewritten { action, .. } if action == "replaced"
        )),
        "replace must be audited: {:?}",
        rig.audit.snapshot(),
    );
    shutdown(&rig);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_whitelist_outside_reference_rejected() {
    let grant = test_grant_full(Scope::Sandbox, vec!["fs".to_owned()], AskRoute::RemoteGui);
    let rig = rig("mcp-rej", grant, |cfg| {
        cfg.mcp_definitions.insert(
            "fs".to_owned(),
            serde_json::json!({ "command": "node", "args": ["fs-server.js"] }),
        );
    })
    .await;
    let mut stream = connect_and_stream(&rig.client, rig.server_peer).await;
    handshake_client(&mut stream).await;
    let line = session_new_with_mcp(
        3,
        serde_json::json!([{ "name": "fs" }, { "name": "evil", "command": "rm" }]),
    );
    send_line(&mut stream, &line).await;
    let first = read_line(&mut stream).await.expect("error response");
    let root: Value = serde_json::from_str(first.trim_end()).expect("json");
    assert_eq!(
        root["error"]["code"], -32_602,
        "wire must see rejection: {first}"
    );
    assert_eq!(root["id"], 3, "rejection must echo the request id");
    assert!(
        rig.audit.contains(|ev| matches!(
            ev,
            AuditEvent::McpRewritten { action, .. } if action == "rejected"
        )),
        "rejection must be audited: {:?}",
        rig.audit.snapshot(),
    );
    shutdown(&rig);
}
