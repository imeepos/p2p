//! admin 端点语义（设计 §5 表）：POST/GET/DELETE /shares。
//! create：入参 {scope, allow_mcp?, ask_route?, ttl_secs, max_activations?, note?}，
//! 出参含 share_id + token 原文 + 链接要素；scope=workspace 未配目录 → 422。
//! list：脱敏（无 token 原文/哈希）；revoke：级联语义（设计 §3）。

use acp_common::policy::{AskRoute, Scope};
use acp_common::{build_share_link, unix_now, ShareSpec, DEFAULT_MAX_ACTIVATIONS};
use serde::Deserialize;
use serde_json::json;
use tokio::net::TcpStream;

use super::admin::{reply, reply_json, AdminDeps};

/// 路由分发：鉴权已在管道层完成，这里只做端点匹配。cors = 白名单内请求 Origin。
pub(super) async fn route(
    tcp: &mut TcpStream,
    deps: &AdminDeps,
    method: &str,
    target: &str,
    body: &[u8],
    cors: Option<&str>,
) {
    let share_prefix = "/shares/";
    match (method, target) {
        ("POST", "/shares") => create_share(tcp, deps, body, cors).await,
        ("GET", "/shares") => list_shares(tcp, deps, cors).await,
        ("DELETE", path) if path.starts_with(share_prefix) => {
            revoke_share(tcp, deps, path.trim_start_matches(share_prefix), cors).await;
        }
        _ => reply(tcp, 404, "Not Found", "{\"error\":\"not-found\"}", cors).await,
    }
}

/// POST /shares：创建（设计 §5）。scope=workspace 未配 workspace-dir → 422。
async fn create_share(tcp: &mut TcpStream, deps: &AdminDeps, body: &[u8], cors: Option<&str>) {
    let parsed: AdminCreateBody = match serde_json::from_slice(body) {
        Ok(parsed) => parsed,
        Err(err) => {
            tracing::warn!(error = %err, "admin: invalid create body");
            reply(
                tcp,
                400,
                "Bad Request",
                "{\"error\":\"invalid-json\"}",
                cors,
            )
            .await;
            return;
        }
    };
    if parsed.ttl_secs == 0 {
        reply(tcp, 400, "Bad Request", "{\"error\":\"invalid-ttl\"}", cors).await;
        return;
    }
    if parsed.max_activations.is_some_and(|max| max == 0) {
        reply(
            tcp,
            400,
            "Bad Request",
            "{\"error\":\"invalid-max-activations\"}",
            cors,
        )
        .await;
        return;
    }
    let spec = ShareSpec {
        scope: parsed.scope,
        allow_mcp: parsed.allow_mcp,
        ask_route: parsed.ask_route.unwrap_or(AskRoute::RemoteGui),
        max_activations: parsed.max_activations.unwrap_or(DEFAULT_MAX_ACTIVATIONS),
        note: parsed.note.unwrap_or_default(),
        ttl_secs: parsed.ttl_secs,
    };
    match deps.service.create(spec, unix_now()) {
        Ok(created) => {
            let share_id = created.entry.share_id.to_string();
            let link = build_share_link(
                &deps.link.peer,
                &deps.link.addrs,
                &created.token,
                created.entry.expires_at_unix,
                &share_id,
            );
            println!("{{\"kind\":\"share-created\",\"share_id\":\"{share_id}\"}}");
            let body = json!({
                "share_id": share_id,
                "token": created.token,
                "link": link,
                "peer": deps.link.peer,
                "addrs": deps.link.addrs,
                "expires_at_unix": created.entry.expires_at_unix,
                "created_at": created.entry.created_at,
                "scope": created.entry.scope,
            });
            reply_json(tcp, 200, "OK", &body, cors).await;
        }
        Err(err) => {
            tracing::warn!(error = %err, "admin: share create rejected");
            let (status, code) = match err {
                super::ShareCreateError::OwnerScope => (400, "owner-scope-not-shareable"),
                super::ShareCreateError::WorkspaceUnconfigured => (422, "workspace-unconfigured"),
                super::ShareCreateError::Store(_) => (500, "store"),
            };
            reply_json(tcp, status, "Error", &json!({ "error": code }), cors).await;
        }
    }
}

/// GET /shares：脱敏列表（无 token 原文/哈希，含 activations/exp/bound/revoke）。
async fn list_shares(tcp: &mut TcpStream, deps: &AdminDeps, cors: Option<&str>) {
    let now = unix_now();
    let shares: Vec<serde_json::Value> = deps
        .service
        .list()
        .iter()
        .map(|entry| {
            json!({
                "share_id": entry.share_id.to_string(),
                "scope": entry.scope,
                "allow_mcp": entry.allow_mcp,
                "ask_route": entry.ask_route,
                "note": entry.note,
                "max_activations": entry.max_activations,
                "activations": entry.activations,
                "expires_at_unix": entry.expires_at_unix,
                "revoked": entry.revoked,
                "bound_peer": entry.bound_peer,
                "created_at": entry.created_at,
                "status": entry.status(now),
            })
        })
        .collect();
    reply_json(tcp, 200, "OK", &json!({ "shares": shares }), cors).await;
}

/// DELETE /shares/{share_id}：撤销（设计 §3 级联语义）。
async fn revoke_share(tcp: &mut TcpStream, deps: &AdminDeps, share_id: &str, cors: Option<&str>) {
    match deps.service.revoke(share_id) {
        Ok(report) => {
            println!(
                "{{\"kind\":\"share-revoked\",\"share_id\":\"{}\"}}",
                report.share_id
            );
            let body = json!({
                "share_id": report.share_id,
                "revoked": true,
                "already_revoked": report.already_revoked,
                "policy_removed": report.policy_removed,
            });
            reply_json(tcp, 200, "OK", &body, cors).await;
        }
        Err(super::RevokeError::Unknown(_)) => {
            reply(tcp, 404, "Not Found", "{\"error\":\"unknown-share\"}", cors).await;
        }
        Err(err) => {
            tracing::error!(error = %err, "admin: share revoke failed");
            reply(
                tcp,
                500,
                "Internal Server Error",
                "{\"error\":\"store\"}",
                cors,
            )
            .await;
        }
    }
}

#[derive(Deserialize)]
struct AdminCreateBody {
    scope: Scope,
    #[serde(default)]
    allow_mcp: Vec<String>,
    #[serde(default)]
    ask_route: Option<AskRoute>,
    ttl_secs: u64,
    #[serde(default)]
    max_activations: Option<u32>,
    #[serde(default)]
    note: Option<String>,
}
