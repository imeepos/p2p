//! acp 域输出面：报告结构（--json 的事实源）与人读文本/表格渲染。
//! 枚举值沿用 acp-common 的 serde 形态（sandbox/remote_gui 等 snake_case）。

use acp_common::policy::{AskRoute, Scope};
use serde::Serialize;

/// allow 报告：created=false 表示条目已存在（本次为 upsert 更新）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowReport {
    pub created: bool,
    pub peer_id: String,
    pub scope: Scope,
    pub allow_mcp: Vec<String>,
    pub ask_route: AskRoute,
    pub granted_at: String,
}

/// deny 报告：removed 恒 true（条目不存在时命令已报错退出）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DenyReport {
    pub removed: bool,
    pub peer_id: String,
}

/// list 条目：策略表全字段透出（含 TOFU 指纹与备注）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListEntry {
    pub peer_id: String,
    pub scope: Scope,
    pub allow_mcp: Vec<String>,
    pub ask_route: AskRoute,
    pub granted_at: String,
    pub fingerprint: String,
    pub note: String,
}

/// list 报告信封。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListReport {
    pub peers: Vec<ListEntry>,
}

/// allow 人读输出：首句结论 + key=value 行（grep 可采集）。
pub fn render_allow(report: &AllowReport) -> String {
    let verdict = if report.created {
        "已授权"
    } else {
        "已更新授权"
    };
    let state = if report.created {
        "新建条目"
    } else {
        "条目已存在，本次为更新"
    };
    let mut lines = vec![format!("{verdict} peer={}（{state}）", report.peer_id)];
    lines.push(format!("scope={}", scope_label(report.scope)));
    lines.push(format!("allow_mcp={}", join_or_dash(&report.allow_mcp)));
    lines.push(format!("ask_route={}", ask_route_label(report.ask_route)));
    lines.push(format!("granted_at={}", report.granted_at));
    lines.join("\n")
}

/// deny 人读输出。
pub fn render_deny(report: &DenyReport) -> String {
    format!("已撤销授权 peer={}（回到默认拒绝）", report.peer_id)
}

/// list 人读输出：对齐表格；空表给明确空态（默认拒绝语义提示）。
pub fn render_list(entries: &[ListEntry]) -> String {
    if entries.is_empty() {
        return "策略表为空（默认拒绝：未列入条目的 peer 一律无授权）".to_owned();
    }
    let header = [
        "PEER",
        "SCOPE",
        "ALLOW_MCP",
        "ASK_ROUTE",
        "GRANTED_AT",
        "FINGERPRINT",
        "NOTE",
    ];
    let mut rows: Vec<Vec<String>> = vec![header.iter().map(|cell| cell.to_string()).collect()];
    rows.extend(entries.iter().map(entry_cells));
    let widths = header
        .iter()
        .enumerate()
        .map(|(i, h)| {
            rows.iter()
                .map(|row| row[i].chars().count())
                .max()
                .unwrap_or(h.len())
        })
        .collect::<Vec<_>>();
    let lines = rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, cell)| format!("{cell:<width$}", width = widths[i]))
                .collect::<Vec<_>>()
                .join("  ")
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>();
    format!("共 {} 条授权\n{}", entries.len(), lines.join("\n"))
}

fn entry_cells(entry: &ListEntry) -> Vec<String> {
    vec![
        entry.peer_id.clone(),
        scope_label(entry.scope).to_owned(),
        join_or_dash(&entry.allow_mcp),
        ask_route_label(entry.ask_route).to_owned(),
        entry.granted_at.clone(),
        or_dash(&entry.fingerprint),
        or_dash(&entry.note),
    ]
}

fn join_or_dash(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_owned()
    } else {
        values.join(",")
    }
}

fn or_dash(value: &str) -> String {
    if value.is_empty() {
        "-".to_owned()
    } else {
        value.to_owned()
    }
}

fn scope_label(scope: Scope) -> &'static str {
    match scope {
        Scope::Sandbox => "sandbox",
        Scope::Workspace => "workspace",
        Scope::Owner => "owner",
    }
}

fn ask_route_label(route: AskRoute) -> &'static str {
    match route {
        AskRoute::RemoteGui => "remote_gui",
        AskRoute::OwnerLocal => "owner_local",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(scope: Scope) -> ListEntry {
        ListEntry {
            peer_id: "PEER1".to_owned(),
            scope,
            allow_mcp: vec!["fs".to_owned(), "web".to_owned()],
            ask_route: AskRoute::RemoteGui,
            granted_at: "2026-09-04T06:00:00Z".to_owned(),
            fingerprint: String::new(),
            note: String::new(),
        }
    }

    #[test]
    fn empty_table_has_explicit_empty_state() {
        assert!(render_list(&[]).starts_with("策略表为空"));
    }

    #[test]
    fn table_lists_all_columns_and_dash_fallbacks() {
        let text = render_list(&[entry(Scope::Sandbox)]);
        for column in [
            "PEER",
            "SCOPE",
            "ALLOW_MCP",
            "ASK_ROUTE",
            "GRANTED_AT",
            "FINGERPRINT",
            "NOTE",
        ] {
            assert!(text.contains(column), "缺列 {column}: {text}");
        }
        assert!(text.contains("fs,web"));
        assert!(text.contains("sandbox"));
        assert!(text.contains("remote_gui"));
    }
}
