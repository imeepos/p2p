//! session/new.mcpServers 安全改写点（设计 §6 MCP 行，全案最重要一行）：
//! 默认整字段剥离；allow_mcp 白名单 peer 仅可按名引用 node 预定义服务定义，
//! 桥替换为 host 侧定义（命令字节永远在 host 手里）；白名单外引用整请求拒绝。
//! 非 session/new 行零改动透传（协议智能不进 Rust 的边界在此保持）。

use std::collections::BTreeMap;

use serde_json::{json, Value};

pub const MCP_REJECT_MESSAGE: &str = "mcp-servers-rejected";
/// JSON-RPC invalid params
pub const MCP_REJECT_CODE: i64 = -32_602;

/// 改写裁决：action 进审计；child_line None = 不转发子进程；wire_error 回给客户端。
#[derive(Debug, PartialEq, Eq)]
pub struct McpOutcome {
    pub action: &'static str,
    pub detail: String,
    pub child_line: Option<Vec<u8>>,
    pub wire_error: Option<String>,
}

pub fn rewrite(
    line: &[u8],
    allow_mcp: &[String],
    definitions: &BTreeMap<String, Value>,
) -> McpOutcome {
    let Ok(mut root) = serde_json::from_slice::<Value>(line) else {
        return passthrough(line);
    };
    let Some(obj) = root.as_object_mut() else {
        return passthrough(line);
    };
    if obj.get("method").and_then(Value::as_str) != Some("session/new") {
        return passthrough(line);
    }
    let Some(servers) = take_mcp_servers(obj) else {
        return passthrough(line);
    };
    if allow_mcp.is_empty() {
        return strip(root, &servers);
    }
    match replace_servers(&servers, allow_mcp, definitions) {
        Ok((names, defined)) => replace(root, names, defined),
        Err(detail) => reject(obj.get("id"), detail),
    }
}

fn passthrough(line: &[u8]) -> McpOutcome {
    McpOutcome {
        action: "untouched",
        detail: String::new(),
        child_line: Some(line.to_vec()),
        wire_error: None,
    }
}

/// 摘除 params.mcpServers；字段不存在视为无事发生。
fn take_mcp_servers(obj: &mut serde_json::Map<String, Value>) -> Option<Value> {
    obj.get_mut("params")?.as_object_mut()?.remove("mcpServers")
}

fn strip(root: Value, servers: &Value) -> McpOutcome {
    let entries = servers.as_array().map_or(0, Vec::len);
    McpOutcome {
        action: "stripped",
        detail: format!("removed mcpServers with {entries} entries"),
        child_line: Some(reserialize(root)),
        wire_error: None,
    }
}

fn replace(root: Value, names: Vec<String>, defined: Value) -> McpOutcome {
    let mut root = root;
    if let Some(params) = root.get_mut("params").and_then(Value::as_object_mut) {
        params.insert("mcpServers".to_owned(), defined);
    }
    McpOutcome {
        action: "replaced",
        detail: format!("substituted host definitions for {names:?}"),
        child_line: Some(reserialize(root)),
        wire_error: None,
    }
}

/// 整请求拒绝：有 id 回 JSON-RPC 错误（客户端可见），notification 静默丢弃仅审计。
fn reject(id: Option<&Value>, detail: String) -> McpOutcome {
    let answerable = id.is_some_and(|id| !id.is_null());
    McpOutcome {
        action: "rejected",
        detail,
        child_line: None,
        wire_error: answerable.then(|| error_response(id.unwrap_or(&Value::Null))),
    }
}

/// 白名单逐项校验：必须按名引用，且 host 有同名预定义定义。
fn replace_servers(
    servers: &Value,
    allow_mcp: &[String],
    definitions: &BTreeMap<String, Value>,
) -> Result<(Vec<String>, Value), String> {
    let entries = servers
        .as_array()
        .ok_or_else(|| format!("mcpServers must be an array, got {servers}"))?;
    let mut names = Vec::with_capacity(entries.len());
    let mut defined = Vec::with_capacity(entries.len());
    for entry in entries {
        let name = entry
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("mcpServers entry must reference by name, got {entry}"))?;
        let allowed = allow_mcp.iter().any(|a| a == name);
        match (allowed, definitions.get(name)) {
            (true, Some(def)) => {
                names.push(name.to_owned());
                defined.push(def.clone());
            }
            (true, None) => {
                return Err(format!(
                    "mcp name '{name}' allowed but not predefined on host"
                ));
            }
            (false, _) => {
                return Err(format!("mcp name '{name}' not in allow_mcp whitelist"));
            }
        }
    }
    Ok((names, Value::Array(defined)))
}

fn error_response(id: &Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": MCP_REJECT_CODE, "message": MCP_REJECT_MESSAGE },
    })
    .to_string()
}

/// 重写后的行必须自带换行（stdin 是行协议）；透传行保持原始字节。
fn reserialize(root: Value) -> Vec<u8> {
    let mut line = root.to_string().into_bytes();
    line.push(b'\n');
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs() -> BTreeMap<String, Value> {
        BTreeMap::from([(
            "fs".to_owned(),
            json!({ "command": "node", "args": ["fs-server.js"] }),
        )])
    }

    fn session_new(params: Value) -> Vec<u8> {
        let mut line = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "session/new",
            "params": params,
        })
        .to_string()
        .into_bytes();
        line.push(b'\n');
        line
    }

    #[test]
    fn default_peer_strips_field() {
        let params = json!({"sessionId": "s", "mcpServers": [{"command": "evil"}]});
        let out = rewrite(&session_new(params), &[], &BTreeMap::new());
        assert_eq!(out.action, "stripped");
        let sent = out.child_line.expect("forward");
        let root: Value = serde_json::from_slice(&sent).expect("json");
        assert!(root["params"].get("mcpServers").is_none());
        assert!(out.wire_error.is_none());
    }

    #[test]
    fn non_target_lines_pass_byte_identical() {
        let ping = b"{\"method\":\"ping\"}\n".to_vec();
        let out = rewrite(&ping, &[], &BTreeMap::new());
        assert_eq!(out.action, "untouched");
        assert_eq!(out.child_line.expect("bytes"), ping);

        let junk = b"not json at all\n".to_vec();
        let out = rewrite(&junk, &["fs".to_owned()], &defs());
        assert_eq!(out.action, "untouched");
        assert_eq!(out.child_line.expect("bytes"), junk);
    }

    #[test]
    fn whitelist_replaces_with_host_definitions() {
        let params = json!({"mcpServers": [{"name": "fs"}]});
        let out = rewrite(&session_new(params), &["fs".to_owned()], &defs());
        assert_eq!(out.action, "replaced");
        let root: Value = serde_json::from_slice(&out.child_line.expect("fwd")).expect("json");
        assert_eq!(root["params"]["mcpServers"][0]["command"], "node");
    }

    #[test]
    fn whitelist_outside_reference_rejects_whole_request() {
        let params = json!({"mcpServers": [{"name": "fs"}, {"name": "evil", "command": "rm"}]});
        let out = rewrite(&session_new(params), &["fs".to_owned()], &defs());
        assert_eq!(out.action, "rejected");
        assert!(out.child_line.is_none());
        let err: Value = serde_json::from_str(&out.wire_error.expect("answerable")).expect("json");
        assert_eq!(err["id"], 1);
        assert_eq!(err["error"]["code"], MCP_REJECT_CODE);
    }

    #[test]
    fn notification_rejection_has_no_wire_answer() {
        let line = b"{\"jsonrpc\":\"2.0\",\"method\":\"session/new\",\"params\":{\"mcpServers\":[{\"name\":\"x\"}]}}\n".to_vec();
        let out = rewrite(&line, &["fs".to_owned()], &defs());
        assert_eq!(out.action, "rejected");
        assert!(out.wire_error.is_none());
    }

    #[test]
    fn non_array_field_is_rejected() {
        let params = json!({"mcpServers": "oops"});
        let out = rewrite(&session_new(params), &["fs".to_owned()], &defs());
        assert_eq!(out.action, "rejected");
    }
}
