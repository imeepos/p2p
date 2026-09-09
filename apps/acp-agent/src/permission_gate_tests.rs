//! decide_gated 单测（authz-role-design §8 权限瀑布行）：闸只在 ask（Forward）
//! 路径被调用；Deny 即 AuthzDenied 本地直拒；read/think 放行与 owner-local
//! 拒绝不触发判定（gate 闭包零调用）。permission.rs 本体行数贴线，测试外置。

use acp_common::AskRoute;
use serde_json::json;

use crate::permission::{self, classify, Decision, GatedDecision};

fn request(kind: &str) -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "session/request_permission",
        "params": {
            "toolCall": { "kind": kind, "title": "demo" },
            "options": [
                { "optionId": "allow-once", "name": "Allow", "kind": "allow_once" },
                { "optionId": "reject-once", "name": "Deny", "kind": "reject_once" },
            ],
        },
    })
}

fn outcome_of(response: &str) -> serde_json::Value {
    let root: serde_json::Value = serde_json::from_str(response).expect("json");
    root["result"]["outcome"].clone()
}

#[test]
fn gate_denied_forward_lands_as_local_reject_without_popup() {
    let req = classify(&request("execute")).expect("permission request");
    let decision = permission::decide_gated(&req, AskRoute::RemoteGui, || false);
    let GatedDecision::AuthzDenied(response) = decision else {
        panic!("execute ask without acp.execute must be locally rejected");
    };
    assert_eq!(
        outcome_of(&response)["outcome"],
        "cancelled",
        "authz denial must be a reject-once answer, never a forwarded ask"
    );
}

#[test]
fn gate_allowed_forward_keeps_the_ask_route() {
    let req = classify(&request("execute")).expect("permission request");
    assert!(matches!(
        permission::decide_gated(&req, AskRoute::RemoteGui, || true),
        GatedDecision::Inner(Decision::Forward)
    ));
}

#[test]
fn static_allow_and_owner_local_never_call_the_gate() {
    let calls = std::cell::Cell::new(0usize);
    let gate = || {
        calls.set(calls.get() + 1);
        true
    };
    let read = classify(&request("read")).expect("permission request");
    assert!(matches!(
        permission::decide_gated(&read, AskRoute::RemoteGui, gate),
        GatedDecision::Inner(Decision::AutoAllow(_))
    ));
    let execute = classify(&request("execute")).expect("permission request");
    assert!(matches!(
        permission::decide_gated(&execute, AskRoute::OwnerLocal, gate),
        GatedDecision::Inner(Decision::OwnerLocal(_))
    ));
    assert_eq!(calls.get(), 0, "non-ask paths must not trigger authz IO");
}
