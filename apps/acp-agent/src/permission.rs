//! request_permission 权限瀑布（设计 §6 工具行）：
//! ① 静态策略先行：toolCall.kind read/think/fetch = allow；
//!    execute/edit/delete 及未知 kind = ask（保守默认）。
//! ② ask 路由：remote_gui = 透传客户端（超时桥代答 reject-once）；
//!    owner_local = 本地审计 + 立即 reject-once 占位（交互面 GUI 波接管）。
//! ③ grant 一次性、永不持久化：桥不落任何许可状态。

use acp_common::AskRoute;
use serde_json::Value;

pub const CANCELLED_OUTCOME: &str = "cancelled";
pub const SELECTED_OUTCOME: &str = "selected";

/// 从子进程行识别权限请求：method 以 request_permission 结尾且携带可应答 id。
pub struct PermissionRequest {
    pub id: Value,
    pub tool_kind: Option<String>,
    pub allow_option: Option<Value>,
}

pub fn classify(root: &Value) -> Option<PermissionRequest> {
    let obj = root.as_object()?;
    let method = obj.get("method")?.as_str()?;
    if !method.ends_with("request_permission") {
        return None;
    }
    let id = obj.get("id")?;
    if id.is_null() {
        return None;
    }
    let params = obj.get("params");
    let tool_kind = params
        .and_then(|p| p.get("toolCall"))
        .and_then(|t| t.get("kind"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let allow_option = params
        .and_then(|p| p.get("options"))
        .and_then(Value::as_array)
        .and_then(|opts| find_allow_option(opts));
    Some(PermissionRequest {
        id: id.clone(),
        tool_kind,
        allow_option,
    })
}

fn find_allow_option(options: &[Value]) -> Option<Value> {
    options
        .iter()
        .find(|o| {
            o.get("kind")
                .and_then(Value::as_str)
                .is_some_and(|k| k.starts_with("allow"))
        })
        .cloned()
}

pub enum Decision {
    /// 静态策略放行：代答选中 allow 选项（不经客户端）。
    AutoAllow(String),
    /// ask 路由 owner_local：本地审计 + reject-once 占位。
    OwnerLocal(String),
    /// ask 路由 remote_gui：透传客户端，登记 outstanding。
    Forward,
}

pub fn decide(req: &PermissionRequest, route: AskRoute) -> Decision {
    if let (Some(kind), Some(option)) = (&req.tool_kind, &req.allow_option) {
        if matches!(kind.as_str(), "read" | "think" | "fetch") {
            return Decision::AutoAllow(selected_response(&req.id, option));
        }
    }
    match route {
        AskRoute::OwnerLocal => Decision::OwnerLocal(rejected_response(&req.id)),
        AskRoute::RemoteGui => Decision::Forward,
    }
}

/// 无人值守代答：reject-once（一次性，GUI 显示为已拒绝）。
pub fn rejected_response(id: &Value) -> String {
    json_response(id, cancelled_outcome())
}

fn cancelled_outcome() -> Value {
    serde_json::json!({ "outcome": { "outcome": CANCELLED_OUTCOME } })
}

fn selected_response(id: &Value, option: &Value) -> String {
    let option_id = option.get("optionId").cloned().unwrap_or(Value::Null);
    let outcome = serde_json::json!({
        "outcome": { "outcome": SELECTED_OUTCOME, "optionId": option_id },
    });
    json_response(id, outcome)
}

fn json_response(id: &Value, result: Value) -> String {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(kind: &str, options: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "session/request_permission",
            "params": {
                "toolCall": { "kind": kind, "title": "demo" },
                "options": options,
            },
        })
    }

    fn options() -> Value {
        json!([
            { "optionId": "allow-once", "name": "Allow", "kind": "allow_once" },
            { "optionId": "reject-once", "name": "Deny", "kind": "reject_once" },
        ])
    }

    #[test]
    fn read_kind_auto_allows_first_allow_option() {
        let req = classify(&request("read", options())).expect("permission request");
        let Decision::AutoAllow(response) = decide(&req, AskRoute::RemoteGui) else {
            panic!("read must auto-allow")
        };
        let root: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(root["result"]["outcome"]["outcome"], SELECTED_OUTCOME);
        assert_eq!(root["result"]["outcome"]["optionId"], "allow-once");
    }

    #[test]
    fn execute_kind_routes_by_ask_route() {
        let req = classify(&request("execute", options())).expect("permission request");
        assert!(matches!(
            decide(&req, AskRoute::RemoteGui),
            Decision::Forward
        ));
        let Decision::OwnerLocal(response) = decide(&req, AskRoute::OwnerLocal) else {
            panic!("owner_local must reject locally")
        };
        let root: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(root["result"]["outcome"]["outcome"], CANCELLED_OUTCOME);
    }

    #[test]
    fn unknown_kind_defaults_to_ask() {
        for kind in ["delete", "move", "other", "surprise"] {
            let req = classify(&request(kind, options())).expect("permission request");
            assert!(matches!(
                decide(&req, AskRoute::RemoteGui),
                Decision::Forward
            ));
        }
    }

    #[test]
    fn missing_allow_option_falls_back_to_ask() {
        let req = classify(&request("read", json!([]))).expect("permission request");
        assert!(matches!(
            decide(&req, AskRoute::RemoteGui),
            Decision::Forward
        ));
    }

    #[test]
    fn non_permission_lines_are_not_classified() {
        for line in [
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            json!({"jsonrpc": "2.0", "method": "session/update", "params": {}}),
            json!({"jsonrpc": "2.0", "method": "session/request_permission", "params": {}}),
        ] {
            assert!(classify(&line).is_none());
        }
    }
}
