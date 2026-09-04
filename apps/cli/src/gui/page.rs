//! gui page/action（GC4）：页面语义协议消费——读当前页 descriptor、执行页面动作。
//! 协议权威：docs/design/gui-control-channel.md 页面协议章与 GUI 前端注册表（GC3）；
//! 本模块只消费控制通道端点，不改 GUI 侧。

use serde_json::{Value, json};

use super::OutputArgs;
use super::channel::Channel;
use super::{open, parse_pairs};
use crate::error::{CliError, CliResult};
use crate::output;

/// GET page/current：当前页 descriptor（人读表格；--json 全量含 args schema 与 state）。
pub async fn show(args: &OutputArgs) -> CliResult<()> {
    let data = open(args)?.get("/page/current").await?;
    output::emit(args.json, &data, &render_page(&data))
}

/// POST page/action：执行页面动作；非当前页默认结构化报错，--navigate 先切页再执行。
pub async fn run(
    out: &OutputArgs,
    page: &str,
    action: &str,
    pairs: &[String],
    navigate: bool,
) -> CliResult<()> {
    let channel = open(out)?;
    if navigate {
        channel.post("/navigate", json!({ "route": page })).await?;
    } else {
        ensure_current_page(&channel, page).await?;
    }
    let body = json!({ "page": page, "action": action, "args": parse_pairs(pairs)? });
    let data = channel.post("/page/action", body).await?;
    let text = serde_json::to_string_pretty(&data)
        .map_err(|e| CliError::Runtime(format!("action 结果序列化失败: {e}")))?;
    output::emit(out.json, &data, &text)
}

/// 非当前页拒绝：结构化错误含 navigate 指引（可读呈现，R2）。
async fn ensure_current_page(channel: &Channel, page: &str) -> CliResult<()> {
    let health = channel.get("/health").await?;
    let route = health["route"].as_str().unwrap_or("");
    if route != page {
        return Err(page_mismatch_error(page, route));
    }
    Ok(())
}

fn page_mismatch_error(page: &str, current: &str) -> CliError {
    CliError::Runtime(format!(
        "页面 {page} 非当前页（当前 route={current}）：先执行 p2pctl gui navigate {page}，或加 --navigate 自动切页"
    ))
}

/// 人读表格：name/description/actions 计数 + 每 action 一行（含 args schema 标注）。
/// state 快照不入文本（--json 全量承载），保持表格 grep 友好。
fn render_page(data: &Value) -> String {
    let name = data["name"].as_str().unwrap_or("?");
    let description = data["description"].as_str().unwrap_or("");
    let actions = data["actions"].as_array();
    let mut lines = vec![
        format!("name={name}"),
        format!("description={description}"),
        format!("actions={}", actions.map_or(0, |a| a.len())),
    ];
    for action in actions.into_iter().flatten() {
        lines.push(render_action(action));
    }
    lines.join("\n")
}

fn render_action(action: &Value) -> String {
    let name = action["name"].as_str().unwrap_or("?");
    let description = action["description"].as_str().unwrap_or("");
    let confirm = if action["confirm"].as_bool().unwrap_or(false) {
        " [confirm]"
    } else {
        ""
    };
    format!("- {name}: {description}{confirm}\n  args: {}", render_args(action))
}

fn render_args(action: &Value) -> String {
    let defs = match action["args"].as_array() {
        Some(d) if !d.is_empty() => d,
        _ => return "（无）".to_string(),
    };
    defs.iter().map(arg_label).collect::<Vec<_>>().join(" ")
}

fn arg_label(def: &Value) -> String {
    let name = def["name"].as_str().unwrap_or("?");
    let ty = def["type"].as_str().unwrap_or("?");
    let required = if def["required"].as_bool().unwrap_or(false) {
        "必填"
    } else {
        "选填"
    };
    format!("{name}({ty},{required})")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_page() -> Value {
        json!({
            "name": "chat",
            "description": "IM 聊天页",
            "actions": [
                { "name": "sendText", "description": "发送文本", "args": [
                    { "name": "peer", "type": "string", "required": true, "description": "好友" },
                    { "name": "text", "type": "string", "required": true, "description": "正文" },
                ]},
                { "name": "removeFriend", "description": "移除好友", "confirm": true, "args": [
                    { "name": "peer", "type": "string", "required": true, "description": "好友" },
                    { "name": "confirm", "type": "boolean", "required": true, "description": "危险确认" },
                ]},
                { "name": "refresh", "description": "刷新", "args": [] },
            ],
            "state": { "friends": 1 },
        })
    }

    #[test]
    fn render_page_lists_actions_args_and_confirm() {
        let text = render_page(&sample_page());
        assert!(text.contains("name=chat"), "{text}");
        assert!(text.contains("description=IM 聊天页"), "{text}");
        assert!(text.contains("actions=3"), "{text}");
        assert!(text.contains("- sendText: 发送文本\n  args: peer(string,必填) text(string,必填)"), "{text}");
        assert!(text.contains("- removeFriend: 移除好友 [confirm]"), "{text}");
        assert!(text.contains("- refresh: 刷新\n  args: （无）"), "{text}");
        // state 不入文本，留 --json 全量
        assert!(!text.contains("friends"), "{text}");
    }

    #[test]
    fn render_page_tolerates_missing_fields() {
        let text = render_page(&json!({}));
        assert_eq!(text, "name=?\ndescription=\nactions=0");
    }

    #[test]
    fn mismatch_error_contains_navigate_hint_and_exit_one() {
        let err = page_mismatch_error("settings", "chat");
        let msg = err.to_string();
        assert!(msg.contains("gui navigate settings"), "{msg}");
        assert!(msg.contains("route=chat"), "{msg}");
        assert!(msg.contains("--navigate"), "{msg}");
        assert_eq!(err.exit_code(), 1);
    }
}
