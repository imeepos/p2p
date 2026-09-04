//! ACP4 权限瀑布回环集成测试（设计 §6 工具行）：read 静态放行、execute ask 透传
//! 客户端批准、ask 超时桥代答 reject-once、owner_local 本地拒绝不透传。
//! 客户端注入的行经回声桩等价于子进程自发，审计断言走 CaptureAudit。

mod common;

use std::time::Duration;

use acp_agent::AuditEvent;
use acp_common::{AskRoute, Scope};
use common::{
    connect_and_stream, handshake_client, permission_approve, permission_request, read_line, rig,
    send_line, shutdown, test_grant_full,
};
use serde_json::Value;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn static_policy_auto_allows_read_kind() {
    let grant = test_grant_full(Scope::Sandbox, Vec::new(), AskRoute::RemoteGui);
    let rig = rig("perm-allow", grant, |_| {}).await;
    let mut stream = connect_and_stream(&rig.client, rig.server_peer).await;
    handshake_client(&mut stream).await;
    send_line(&mut stream, &permission_request(7, "read")).await;
    let only = read_line(&mut stream).await.expect("auto-allow response");
    let root: Value = serde_json::from_str(only.trim_end()).expect("json");
    assert_eq!(
        root["result"]["outcome"],
        serde_json::json!({ "outcome": "selected", "optionId": "allow-once" }),
        "read kind must be auto-allowed with the first allow option: {only}",
    );
    assert!(
        rig.audit.contains(|ev| matches!(
            ev,
            AuditEvent::PermissionActed { action, .. } if action == "auto-allowed"
        )),
        "auto-allow must be audited: {:?}",
        rig.audit.snapshot(),
    );
    shutdown(&rig);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ask_routes_to_remote_gui_and_settles_on_approval() {
    let grant = test_grant_full(Scope::Sandbox, Vec::new(), AskRoute::RemoteGui);
    let rig = rig("perm-ask", grant, |_| {}).await;
    let mut stream = connect_and_stream(&rig.client, rig.server_peer).await;
    handshake_client(&mut stream).await;
    send_line(&mut stream, &permission_request(8, "execute")).await;
    let forwarded = read_line(&mut stream).await.expect("forwarded request");
    let req: Value = serde_json::from_str(forwarded.trim_end()).expect("json");
    assert_eq!(req["id"], 8, "execute kind must reach the remote operator");
    send_line(&mut stream, &permission_approve(8, "allow-once")).await;
    let settled = read_line(&mut stream).await.expect("approval echo");
    let res: Value = serde_json::from_str(settled.trim_end()).expect("json");
    assert_eq!(res["result"]["outcome"]["optionId"], "allow-once");
    assert!(
        rig.audit.contains(|ev| matches!(
            ev,
            AuditEvent::PermissionActed { action, .. } if action == "forwarded"
        )),
        "forward must be audited: {:?}",
        rig.audit.snapshot(),
    );
    shutdown(&rig);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ask_times_out_into_reject_once() {
    let grant = test_grant_full(Scope::Sandbox, Vec::new(), AskRoute::RemoteGui);
    let rig = rig("perm-timeout", grant, |_| {}).await;
    let mut stream = connect_and_stream(&rig.client, rig.server_peer).await;
    handshake_client(&mut stream).await;
    send_line(&mut stream, &permission_request(9, "delete")).await;
    let forwarded = read_line(&mut stream).await.expect("forwarded request");
    assert!(forwarded.contains("request_permission"));
    let answered = tokio::time::timeout(Duration::from_secs(6), read_line(&mut stream)).await;
    let rejected = answered
        .expect("timeout must fire within budget")
        .expect("line");
    let root: Value = serde_json::from_str(rejected.trim_end()).expect("json");
    assert_eq!(
        root["result"]["outcome"]["outcome"], "cancelled",
        "unanswered ask must be rejected once: {rejected}",
    );
    assert!(
        rig.audit.contains(|ev| matches!(
            ev,
            AuditEvent::PermissionActed { action, .. } if action == "timeout-rejected"
        )),
        "timeout rejection must be audited: {:?}",
        rig.audit.snapshot(),
    );
    shutdown(&rig);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_local_route_rejects_without_client() {
    let grant = test_grant_full(Scope::Sandbox, Vec::new(), AskRoute::OwnerLocal);
    let rig = rig("perm-owner", grant, |_| {}).await;
    let mut stream = connect_and_stream(&rig.client, rig.server_peer).await;
    handshake_client(&mut stream).await;
    send_line(&mut stream, &permission_request(10, "edit")).await;
    let only = read_line(&mut stream).await.expect("local reject echo");
    let root: Value = serde_json::from_str(only.trim_end()).expect("json");
    assert_eq!(
        root["result"]["outcome"]["outcome"], "cancelled",
        "owner_local must answer reject-once locally: {only}",
    );
    assert!(
        root.get("method").is_none(),
        "the request itself must never reach the remote client: {only}",
    );
    assert!(
        rig.audit.contains(|ev| matches!(
            ev,
            AuditEvent::PermissionActed { action, .. } if action == "owner-local"
        )),
        "owner-local routing must be audited: {:?}",
        rig.audit.snapshot(),
    );
    shutdown(&rig);
}
