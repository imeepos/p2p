//! A2A agent 管理 admin 端点（a2a-over-p2p-design §3/§8.2 我的视图数据源）：
//! GET/POST /a2a/agents，PUT/DELETE /a2a/agents/{id}。变更即向订阅者广播
//! push/remove（design §7.2/§7.4）。鉴权与 CORS 由 share admin 管道层承担。

use std::sync::Arc;

use a2a::{CardFrame, Visibility};
use serde::Deserialize;
use serde_json::json;
use tokio::net::TcpStream;

use a2a::create_invite;
use uuid::Uuid;

use crate::a2a::handler::Subscribers;
use crate::a2a::invites::InviteStore;

/// 管理上下文：簿 + 订阅表 + 邀请簿（handler 共享同一实例，变更即广播）。
pub struct A2aAdminCtx {
    pub agents: Arc<crate::a2a::agents::AgentStore>,
    pub invites: Arc<InviteStore>,
    pub subscribers: Arc<Subscribers>,
    pub keypair: p2p_identity::Keypair,
    pub host_peer: String,
}

/// 路由分发（鉴权已在管道层完成）。返回 true 表示命中 a2a 路由。
pub async fn route(
    tcp: &mut TcpStream,
    ctx: &A2aAdminCtx,
    method: &str,
    target: &str,
    body: &[u8],
    cors: Option<&str>,
) -> bool {
    let prefix = "/a2a/agents";
    match (method, target) {
        ("GET", "/a2a/agents") => {
            list_agents(tcp, ctx, cors).await;
            true
        }
        ("POST", "/a2a/agents") => {
            create_agent(tcp, ctx, body, cors).await;
            true
        }
        ("PUT", path) if path.starts_with(prefix) => {
            update_agent(tcp, ctx, path.trim_start_matches(prefix), body, cors).await;
            true
        }
        ("DELETE", path) if path.starts_with(prefix) => {
            remove_agent(tcp, ctx, path.trim_start_matches(prefix), cors).await;
            true
        }
        ("POST", path) if path.starts_with(prefix) && path.ends_with("/invite") => {
            let mid = path.trim_start_matches(prefix).trim_start_matches('/');
            let agent_id = mid.trim_end_matches("/invite");
            invite_agent(tcp, ctx, agent_id, body, cors).await;
            true
        }
        _ => false,
    }
}

async fn list_agents(tcp: &mut TcpStream, ctx: &A2aAdminCtx, cors: Option<&str>) {
    let agents = ctx.agents.list();
    let body = json!({ "agents": agents });
    reply_json(tcp, 200, "OK", &body, cors).await;
}

async fn create_agent(tcp: &mut TcpStream, ctx: &A2aAdminCtx, body: &[u8], cors: Option<&str>) {
    let parsed: CreateBody = match serde_json::from_slice(body) {
        Ok(parsed) => parsed,
        Err(err) => {
            tracing::warn!(target: "a2a_audit", error = %err, "a2a admin: invalid create body");
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
    let visibility = parsed.visibility.unwrap_or(Visibility::Private);
    match ctx.agents.create(
        parsed.agent_id,
        parsed.name,
        parsed.description,
        parsed.skills.unwrap_or_default(),
        visibility,
        unix_now(),
    ) {
        Ok(def) => {
            broadcast_change(ctx, &def.agent_id, true);
            reply_json(
                tcp,
                200,
                "OK",
                &serde_json::to_value(def).unwrap_or_default(),
                cors,
            )
            .await;
        }
        Err(err) => {
            tracing::warn!(target: "a2a_audit", error = %err, "a2a admin: create rejected");
            reply(
                tcp,
                422,
                "Unprocessable",
                &format!("{{\"error\":\"{err}\"}}"),
                cors,
            )
            .await;
        }
    }
}

async fn update_agent(
    tcp: &mut TcpStream,
    ctx: &A2aAdminCtx,
    id_path: &str,
    body: &[u8],
    cors: Option<&str>,
) {
    let agent_id = id_path.trim_start_matches('/');
    let parsed: UpdateBody = match serde_json::from_slice(body) {
        Ok(parsed) => parsed,
        Err(err) => {
            reply(
                tcp,
                400,
                "Bad Request",
                "{\"error\":\"invalid-json\"}",
                cors,
            )
            .await;
            let _ = err;
            return;
        }
    };
    let result = if let Some(visibility) = parsed.visibility {
        ctx.agents.set_visibility(agent_id, visibility)
    } else if let Some(enabled) = parsed.enabled {
        ctx.agents.set_enabled(agent_id, enabled)
    } else {
        Err(crate::a2a::agents::StoreError::Unknown(agent_id.to_owned()))
    };
    match result {
        Ok(def) => {
            broadcast_change(ctx, agent_id, false);
            reply_json(
                tcp,
                200,
                "OK",
                &serde_json::to_value(def).unwrap_or_default(),
                cors,
            )
            .await;
        }
        Err(err) => {
            let code = if matches!(err, crate::a2a::agents::StoreError::Unknown(_)) {
                404
            } else {
                422
            };
            reply(
                tcp,
                code,
                "Error",
                &format!("{{\"error\":\"{err}\"}}"),
                cors,
            )
            .await;
        }
    }
}

async fn remove_agent(tcp: &mut TcpStream, ctx: &A2aAdminCtx, id_path: &str, cors: Option<&str>) {
    let agent_id = id_path.trim_start_matches('/');
    match ctx.agents.remove(agent_id) {
        Ok(def) => {
            // 下架广播：removed 键形 hostPeer/agentId（design §7.4）
            let key = format!("{}/{}", ctx.host_peer, def.agent_id);
            ctx.subscribers.notify(CardFrame::Push {
                v: a2a::CARD_FRAME_VERSION,
                id: 0,
                cards: vec![],
                removed: vec![key],
            });
            reply_json(tcp, 200, "OK", &json!({ "removed": def.agent_id }), cors).await;
        }
        Err(err) => {
            let code = if matches!(err, crate::a2a::agents::StoreError::Unknown(_)) {
                404
            } else {
                422
            };
            reply(
                tcp,
                code,
                "Error",
                &format!("{{\"error\":\"{err}\"}}"),
                cors,
            )
            .await;
        }
    }
}

/// 变更广播：重签当前可见卡全集 push（订阅侧按 version 替换/removed 除名）。
fn broadcast_change(ctx: &A2aAdminCtx, agent_id: &str, created: bool) {
    let now = unix_now();
    let cards = ctx
        .agents
        .signed_cards_for(&ctx.keypair, &ctx.host_peer, true, &[], now)
        .into_iter()
        .filter(|c| created || c.0.payload.agent_id != agent_id)
        .collect::<Vec<_>>();
    ctx.subscribers.notify(CardFrame::Push {
        v: a2a::CARD_FRAME_VERSION,
        id: 0,
        cards,
        removed: vec![],
    });
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBody {
    agent_id: Option<String>,
    name: String,
    description: String,
    skills: Option<Vec<a2a::AgentSkill>>,
    visibility: Option<Visibility>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateBody {
    visibility: Option<Visibility>,
    enabled: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InviteBody {
    invitee_peer: String,
    expiry_secs: Option<u64>,
}

/// POST /a2a/agents/{id}/invite：生成签名邀请帧，落盘邀请簿，返回帧 JSON。
/// GUI 用此帧构造分享链接或经 P2P 通道投递给 invitee。
async fn invite_agent(
    tcp: &mut TcpStream,
    ctx: &A2aAdminCtx,
    agent_id: &str,
    body: &[u8],
    cors: Option<&str>,
) {
    let agent_id = agent_id.trim_start_matches('/');
    let Some(def) = ctx.agents.get(agent_id) else {
        reply(tcp, 404, "Not Found", &serde_json::to_string(&json!({"error":"unknown-agent"})).unwrap(), cors).await;
        return;
    };
    let parsed: InviteBody = match serde_json::from_slice(body) {
        Ok(p) => p,
        Err(err) => {
            tracing::warn!(target: "a2a_audit", error = %err, "a2a admin: invalid invite body");
            reply(tcp, 400, "Bad Request", &serde_json::to_string(&json!({"error":"invalid-json"})).unwrap(), cors).await;
            return;
        }
    };
    let now = unix_now();
    let expiry = parsed.expiry_secs.unwrap_or(a2a::INVITE_EXPIRY_DEFAULT_SECS);
    let card = match crate::a2a::agents::AgentStore::card_of(&def, &ctx.host_peer) {
        Some(c) => c,
        None => {
            reply(tcp, 422, "Unprocessable", &serde_json::to_string(&json!({"error":"agent-disabled"})).unwrap(), cors).await;
            return;
        }
    };
    let signed_card = match a2a::card::SignedCard::sign(card, &ctx.keypair, now) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(target: "a2a_audit", error = %e, "a2a admin: card sign failed");
            reply(tcp, 500, "Internal Server Error", &serde_json::to_string(&json!({"error":"sign-failed"})).unwrap(), cors).await;
            return;
        }
    };
    let nonce = Uuid::new_v4().to_string();
    let frame = match create_invite(signed_card, nonce.clone(), &parsed.invitee_peer, expiry, &ctx.keypair, now) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(target: "a2a_audit", error = %e, "a2a admin: invite creation failed");
            reply(tcp, 422, "Unprocessable", &format!("{{\"error\":\"{e}\"}}"), cors).await;
            return;
        }
    };
    let entry = crate::a2a::invites::InviteEntry {
        nonce,
        agent_id: agent_id.to_owned(),
        host_peer: ctx.host_peer.clone(),
        invitee_peer: parsed.invitee_peer,
        expiry: frame.payload.expiry,
        issued_at: frame.issued_at,
        status: crate::a2a::invites::InviteStatus::Pending,
        receipt_sig: None,
    };
    if let Err(e) = ctx.invites.insert(entry) {
        tracing::warn!(target: "a2a_audit", error = %e, "a2a admin: invite persist failed");
        reply(tcp, 500, "Internal Server Error", &format!("{{\"error\":\"{e}\"}}"), cors).await;
        return;
    }
    let frame_json = serde_json::to_value(&frame).unwrap_or_default();
    println!("{{\"kind\":\"a2a-invite-created\",\"agent_id\":\"{}\"}}", agent_id);
    reply_json(tcp, 200, "OK", &json!({ "invite": frame_json }), cors).await;
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// reply/reply_json 复用 share admin 管道（pub(crate) 后跨模块共用）。
use crate::share::admin::{reply, reply_json};

#[cfg(test)]
mod admin_tests {
    use super::*;

    #[test]
    fn create_body_parses_visibility_default_absent() {
        let json = r#"{"name":"test","description":"desc"}"#;
        let body: CreateBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.visibility, None);
        assert_eq!(body.name, "test");
    }
}