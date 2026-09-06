//! acp share 输出面：报告结构（JSON 事实源，snake_case 与设计 §6 契约一致）与人读表格。

use acp_common::policy::{AskRoute, Scope};
use serde::Serialize;

/// create 报告（设计 §6 冻结字段：share_id/token/link）。
#[derive(Debug, Serialize)]
pub struct ShareCreateReport {
    pub share_id: String,
    pub token: String,
    pub link: String,
    pub peer: String,
    pub addrs: Vec<String>,
    pub scope: Scope,
    pub expires_at_unix: u64,
    pub created_at: String,
}

/// list 条目：脱敏（无 token 原文/哈希），status 徽章口径与 admin GET 一致。
#[derive(Debug, Serialize)]
pub struct ShareListEntry {
    pub share_id: String,
    pub scope: Scope,
    pub allow_mcp: Vec<String>,
    pub ask_route: AskRoute,
    pub note: String,
    pub max_activations: u32,
    pub activations: u32,
    pub expires_at_unix: u64,
    pub revoked: bool,
    pub bound_peer: Option<String>,
    pub created_at: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct ShareListReport {
    pub shares: Vec<ShareListEntry>,
}

/// revoke 报告。
#[derive(Debug, Serialize)]
pub struct ShareRevokeReport {
    pub share_id: String,
    pub revoked: bool,
    pub already_revoked: bool,
    pub policy_removed: bool,
}

pub(super) fn render_list(entries: &[ShareListEntry]) -> String {
    if entries.is_empty() {
        return "分享台账为空（用 acp share create 创建）".to_owned();
    }
    let header = [
        "SHARE_ID", "SCOPE", "STATUS", "ACT", "MAX", "EXPIRES", "BOUND", "NOTE",
    ];
    let rows: Vec<Vec<String>> = std::iter::once(header.iter().map(|h| h.to_string()).collect())
        .chain(entries.iter().map(|e| {
            vec![
                e.share_id.clone(),
                scope_label(e.scope).to_owned(),
                e.status.clone(),
                e.activations.to_string(),
                e.max_activations.to_string(),
                e.expires_at_unix.to_string(),
                e.bound_peer.clone().unwrap_or_else(|| "-".to_owned()),
                if e.note.is_empty() {
                    "-".to_owned()
                } else {
                    e.note.clone()
                },
            ]
        }))
        .collect();
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
    format!("共 {} 条分享\n{}", entries.len(), lines.join("\n"))
}

fn scope_label(scope: Scope) -> &'static str {
    match scope {
        Scope::Sandbox => "sandbox",
        Scope::Workspace => "workspace",
        Scope::Owner => "owner",
    }
}
