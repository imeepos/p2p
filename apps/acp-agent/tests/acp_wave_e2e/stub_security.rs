//! 波次场景 ⑧（stub 模式，设计 §6 MCP 行——全案最重要一行）：
//! 默认 peer 的 session/new.mcpServers 整字段剥离；allow_mcp 白名单 peer 只能
//! 按名引用，桥把数组整替为 host 侧预定义定义（命令字节永远在 host 手里）。

use acp_common::{AskRoute, Scope};
use serde_json::Value;

use acp_agent::AuditEvent;

use crate::common::{
    connect_and_stream, handshake_client, read_line, rig, send_line, session_new_with_mcp,
    shutdown, test_grant_full,
};

/// ⑧a 默认剥离：mcpServers 到不了子进程（桩回声里无该字段），动作留审计。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn s8a_default_peer_strips_mcp_servers() {
    let grant = test_grant_full(Scope::Sandbox, Vec::new(), AskRoute::RemoteGui);
    let r = rig("wave-s8a", grant, |_| {}).await;
    let mut stream = connect_and_stream(&r.client, r.server_peer).await;
    handshake_client(&mut stream).await;
    send_line(
        &mut stream,
        &session_new_with_mcp(9, serde_json::json!([{ "command": "evil", "args": ["-x"] }])),
    )
    .await;
    let echoed = read_line(&mut stream).await.expect("echo");
    let root: Value = serde_json::from_str(echoed.trim_end()).expect("json");
    assert!(
        root["params"].get("mcpServers").is_none(),
        "mcpServers must be stripped by default: {echoed}",
    );
    assert!(
        r.audit.contains(|ev| matches!(
            ev,
            AuditEvent::McpRewritten { action, .. } if action == "stripped"
        )),
        "strip must be audited: {:?}",
        r.audit.snapshot(),
    );
    shutdown(&r);
}

/// ⑧b 白名单按名引用：{"name":"fs"} 被整替为 host 预定义定义，越名不可达。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn s8b_whitelist_peer_gets_host_definitions_by_name() {
    let grant = test_grant_full(Scope::Sandbox, vec!["fs".to_owned()], AskRoute::RemoteGui);
    let r = rig("wave-s8b", grant, |cfg| {
        cfg.mcp_definitions.insert(
            "fs".to_owned(),
            serde_json::json!({ "command": "node", "args": ["fs-server.js"] }),
        );
    })
    .await;
    let mut stream = connect_and_stream(&r.client, r.server_peer).await;
    handshake_client(&mut stream).await;
    send_line(
        &mut stream,
        &session_new_with_mcp(10, serde_json::json!([{ "name": "fs" }])),
    )
    .await;
    let echoed = read_line(&mut stream).await.expect("echo");
    let root: Value = serde_json::from_str(echoed.trim_end()).expect("json");
    assert_eq!(
        root["params"]["mcpServers"][0]["command"], "node",
        "reference by name must be replaced with the host definition: {echoed}",
    );
    assert!(
        r.audit.contains(|ev| matches!(
            ev,
            AuditEvent::McpRewritten { action, .. } if action == "replaced"
        )),
        "replace must be audited: {:?}",
        r.audit.snapshot(),
    );
    shutdown(&r);
}
