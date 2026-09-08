//! 桥内权限处置（a2a-over-p2p-design §9 矩阵，桥内闭环不出 wire）：
//! public/private agent 走 OwnerLocal（think 桥代答，read/fetch/execute/edit/
//! delete 一律本地拒绝占位，绝不 RemoteGui）；local agent（owner loopback）走
//! RemoteGui 路由——v1 无 GUI 应答通道时由超时代答 reject-once 兜底。

use std::time::Instant;

use serde_json::Value;
use tokio::io::AsyncWriteExt;

use crate::audit::AuditEvent;
use crate::permission::{self, Decision};

use super::bridge::{BridgeParams, Outstanding, PromptCtx};

pub(crate) async fn answer_permission(
    params: &BridgeParams,
    ctx: &mut PromptCtx,
    req: permission::PermissionRequest,
) {
    let decision = permission::decide_scoped(&req, ask_route(params), params.visibility_local);
    let (response, action) = match decision {
        Decision::AutoAllow(response) => (response, "auto-allowed"),
        Decision::OwnerLocal(response) => (response, "owner-local"),
        Decision::Forward => {
            audit_acted(params, "forwarded", &req.tool_kind);
            ctx.outstanding.push(Outstanding {
                id: req.id,
                deadline: Instant::now() + params.permission_timeout,
            });
            return;
        }
    };
    audit_acted(params, action, &req.tool_kind);
    let mut bytes = response.into_bytes();
    bytes.push(b'\n');
    if let Err(err) = ctx.stdin.write_all(&bytes).await {
        tracing::warn!(task = %params.task_id, error = %err, "permission answer write failed");
        return;
    }
    let _ = ctx.stdin.flush().await;
}

/// 挂起权限超时：代答 reject-once（owner 不在场即超时拒绝，§9 拍板 Q3）。
pub(crate) async fn on_tick(params: &BridgeParams, ctx: &mut PromptCtx) {
    let now = Instant::now();
    let due: Vec<Value> = ctx
        .outstanding
        .iter()
        .filter(|o| o.deadline <= now)
        .map(|o| o.id.clone())
        .collect();
    for id in due {
        let mut bytes = permission::rejected_response(&id).into_bytes();
        bytes.push(b'\n');
        if ctx.stdin.write_all(&bytes).await.is_err() {
            tracing::warn!(task = %params.task_id, "timeout reject write failed");
            continue;
        }
        let _ = ctx.stdin.flush().await;
        ctx.outstanding.retain(|o| o.id != id);
        audit_acted(params, "timeout-rejected", &Some("ask-window".to_owned()));
    }
}

/// §9 红线：public/private 绝不 RemoteGui（陌生人批准 execute = RCE）。
fn ask_route(params: &BridgeParams) -> acp_common::AskRoute {
    if params.visibility_local {
        acp_common::AskRoute::RemoteGui
    } else {
        acp_common::AskRoute::OwnerLocal
    }
}

fn audit_acted(params: &BridgeParams, action: &str, tool_kind: &Option<String>) {
    params.audit.record(AuditEvent::PermissionActed {
        peer: params.peer.clone(),
        conn: params.task_id.clone(),
        action: action.to_owned(),
        detail: tool_kind.clone().unwrap_or_else(|| "unknown".into()),
    });
}
