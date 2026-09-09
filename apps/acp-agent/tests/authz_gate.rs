//! authz 闸集成测试（authz-role-design §8 ACP 两行）：准入双查（策略表 且
//! acp.session）、权限瀑布 execute ask 前置 acp.execute、owner scope 零改动
//! （红线 1）。台架绑定语义见 common::authz_fixture（§9 映射）。

mod common;

use acp_agent::AuditEvent;
use acp_common::{AskRoute, Scope, ServerHello};
use common::{
    connect_and_stream, handshake_client, permission_request, read_line, rig, rig_bound, send_line,
    shutdown, test_grant_full,
};
use serde_json::Value;

/// 有策略表无绑定 → 准入双查拒绝（绑定缺失即拒，§9；wire 只见 peer-not-allowed）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn policy_entry_without_binding_is_denied() {
    let grant = test_grant_full(Scope::Sandbox, Vec::new(), AskRoute::RemoteGui);
    let rig = rig_bound("authz-unbound", grant, |_| {}, None).await;
    let mut stream = connect_and_stream(&rig.client, rig.server_peer).await;
    let reply = handshake_client(&mut stream).await;
    assert_eq!(
        reply,
        ServerHello::Denied {
            denied: "peer-not-allowed".to_owned(),
        },
        "missing authz binding must deny admission"
    );
    assert!(
        rig.audit.contains(|ev| matches!(
            ev,
            AuditEvent::AuthzDenied {
                perm: "acp.session",
                ..
            }
        )),
        "session gate denial must be audited: {:?}",
        rig.audit.snapshot(),
    );
    shutdown(&rig);
}

/// 双闸齐（策略表 + guest 绑定）→ 正常 ready。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn double_gate_allows_when_binding_present() {
    let grant = test_grant_full(Scope::Sandbox, Vec::new(), AskRoute::RemoteGui);
    let rig = rig("authz-ok", grant, |_| {}).await;
    let mut stream = connect_and_stream(&rig.client, rig.server_peer).await;
    let reply = handshake_client(&mut stream).await;
    assert!(
        matches!(reply, ServerHello::Ready { .. }),
        "policy + binding must pass the double gate: {reply:?}"
    );
    shutdown(&rig);
}

/// execute ask 无 acp.execute（guest）→ 本地直拒不弹窗（wire 无透传请求）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn execute_ask_without_acp_execute_is_rejected_locally() {
    let grant = test_grant_full(Scope::Sandbox, Vec::new(), AskRoute::RemoteGui);
    let rig = rig("authz-exec-guest", grant, |_| {}).await;
    let mut stream = connect_and_stream(&rig.client, rig.server_peer).await;
    handshake_client(&mut stream).await;
    send_line(&mut stream, &permission_request(11, "execute")).await;
    let only = read_line(&mut stream).await.expect("local reject echo");
    let root: Value = serde_json::from_str(only.trim_end()).expect("json");
    assert_eq!(
        root["result"]["outcome"]["outcome"], "cancelled",
        "gate-denied execute must be rejected locally: {only}"
    );
    assert!(
        root.get("method").is_none(),
        "the request must never be forwarded to the client: {only}"
    );
    assert!(
        rig.audit.contains(|ev| matches!(
            ev,
            AuditEvent::PermissionActed { action, .. } if action == "authz-denied"
        )),
        "gate denial must be audited: {:?}",
        rig.audit.snapshot(),
    );
    shutdown(&rig);
}

/// execute ask 有 acp.execute（operator）→ 走既有 ask 路由（Forward）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn execute_ask_with_acp_execute_still_forwards() {
    let grant = test_grant_full(Scope::Sandbox, Vec::new(), AskRoute::RemoteGui);
    let rig = rig_bound("authz-exec-op", grant, |_| {}, Some("operator")).await;
    let mut stream = connect_and_stream(&rig.client, rig.server_peer).await;
    handshake_client(&mut stream).await;
    send_line(&mut stream, &permission_request(12, "execute")).await;
    let forwarded = read_line(&mut stream).await.expect("forwarded request");
    assert!(
        forwarded.contains("request_permission"),
        "operator ask must keep the existing ask route: {forwarded}"
    );
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

/// owner scope 条目不进 authz（红线 1）：无绑定仍准入，execute ask 零改动透传。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_scope_bypasses_authz_gates() {
    let grant = test_grant_full(Scope::Owner, Vec::new(), AskRoute::RemoteGui);
    let rig = rig_bound("authz-owner", grant, |_| {}, None).await;
    let mut stream = connect_and_stream(&rig.client, rig.server_peer).await;
    let reply = handshake_client(&mut stream).await;
    assert!(
        matches!(reply, ServerHello::Ready { .. }),
        "owner scope must pass without any authz binding: {reply:?}"
    );
    send_line(&mut stream, &permission_request(13, "execute")).await;
    let forwarded = read_line(&mut stream).await.expect("forwarded request");
    assert!(
        forwarded.contains("request_permission"),
        "owner execute ask must be untouched by the gate: {forwarded}"
    );
    assert!(
        !rig.audit
            .contains(|ev| matches!(ev, AuditEvent::AuthzDenied { .. })),
        "owner path must never consult authz: {:?}",
        rig.audit.snapshot(),
    );
    shutdown(&rig);
}
