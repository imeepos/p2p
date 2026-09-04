//! GC3 页面语义协议：GET /page/current 与 POST /page/action（契约见 §9）。
//! server 经 webview eval 触发前端桥（window.__P2P_PAGES__），前端以
//! control-page-reply 事件回执（requestId 关联）；本模块只做请求编排与
//! 超时/结构化错误，不持有页面语义（前端注册表是唯一事实源）。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, MutexGuard};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{Listener, Runtime};

use super::server::ApiErr;
use super::{ControlCtx, ROUTES};

/// descriptor 契约版本（与前端 page-registry 的 PAGE_SCHEMA_VERSION 同步演进）。
pub const SCHEMA_VERSION: u32 = 1;
/// 回执等待上限（契约 ≤5s）：前端未回执即结构化超时，不静默。
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

static REQUEST_SEQ: AtomicU64 = AtomicU64::new(0);

/// 服务端生成的关联 id（GET /page/current 未带 requestId query 时）。
pub fn new_request_id() -> String {
    format!(
        "srv-{}-{}",
        std::process::id(),
        REQUEST_SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

/// GET /page/current：当前页 descriptor（含 schemaVersion）。
pub fn page_current<R: Runtime>(ctx: &ControlCtx<R>, request_id: &str) -> Result<Value, ApiErr> {
    let page = ctx.current_route();
    let request = json!({ "requestId": request_id, "kind": "describe", "page": page });
    let reply = round_trip(ctx, request_id, &request)?;
    Ok(json!({
        "schemaVersion": SCHEMA_VERSION,
        "page": page,
        "descriptor": reply.get("data").cloned().unwrap_or(Value::Null),
    }))
}

/// POST /page/action{page,action,args?,requestId}：执行回包 {requestId,result}。
pub fn page_action<R: Runtime>(ctx: &ControlCtx<R>, body: &Value) -> Result<Value, ApiErr> {
    let page = str_field(body, "page")?;
    let action = str_field(body, "action")?;
    let request_id = str_field(body, "requestId")?;
    if !ROUTES.contains(&page) {
        return Err(ApiErr::new(
            404,
            "PAGE_NOT_FOUND",
            format!("未知页面 \"{page}\"，可用: {}", ROUTES.join("/")),
        ));
    }
    let request = json!({
        "requestId": request_id,
        "kind": "execute",
        "page": page,
        "action": action,
        "args": body.get("args").cloned().unwrap_or(json!({})),
    });
    let reply = round_trip(ctx, request_id, &request)?;
    Ok(json!({
        "requestId": request_id,
        "result": reply.get("data").cloned().unwrap_or(Value::Null),
    }))
}

/// eval 触发前端桥 → 等待 control-page-reply；超时结构化报错（不静默）。
fn round_trip<R: Runtime>(
    ctx: &ControlCtx<R>,
    request_id: &str,
    request: &Value,
) -> Result<Value, ApiErr> {
    let (tx, rx) = mpsc::channel();
    register_pending(ctx, request_id, tx)?;
    trigger_frontend(ctx, request);
    match rx.recv_timeout(REPLY_TIMEOUT) {
        Ok(reply) if reply.get("ok").and_then(Value::as_bool) == Some(true) => Ok(reply),
        Ok(reply) => Err(protocol_error(&reply)),
        Err(_) => {
            drop_pending(ctx, request_id);
            let message = format!(
                "前端未在 {}s 内回执请求 {request_id}（webview 未就绪或页面桥未安装）",
                REPLY_TIMEOUT.as_secs()
            );
            tracing::error!("control: page 请求超时: {message}");
            Err(ApiErr::new(500, "PAGE_TIMEOUT", message))
        }
    }
}

/// 前端结构化拒绝 → 按错误码映射 HTTP 状态，原样透传 code/message。
fn protocol_error(reply: &Value) -> ApiErr {
    let code = reply
        .pointer("/error/code")
        .and_then(Value::as_str)
        .unwrap_or("ACTION_FAILED");
    let message = reply
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or("前端拒绝或执行失败")
        .to_string();
    ApiErr::new(status_of(code), code, message)
}

fn status_of(code: &str) -> u16 {
    match code {
        "PAGE_NOT_FOUND" | "PAGE_NOT_REGISTERED" | "ACTION_NOT_FOUND" => 404,
        "ACTION_CONFIRM_REQUIRED" | "ARG_MISSING" | "ARG_TYPE_MISMATCH" | "INVALID_REQUEST" => 400,
        _ => 500,
    }
}

fn str_field<'a>(body: &'a Value, name: &str) -> Result<&'a str, ApiErr> {
    body.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| ApiErr::new(400, "INVALID_REQUEST", format!("缺少 {name} 字段（字符串）")))
}

fn register_pending<R: Runtime>(
    ctx: &ControlCtx<R>,
    request_id: &str,
    tx: mpsc::Sender<Value>,
) -> Result<(), ApiErr> {
    page_replies(ctx)?.insert(request_id.to_string(), tx);
    Ok(())
}

/// 超时路径回收登记项；迟到回执会命中 listener 的无主分支留 warn。
fn drop_pending<R: Runtime>(ctx: &ControlCtx<R>, request_id: &str) {
    if let Ok(mut pending) = page_replies(ctx) {
        pending.remove(request_id);
    }
}

fn page_replies<R: Runtime>(
    ctx: &ControlCtx<R>,
) -> Result<MutexGuard<'_, HashMap<String, mpsc::Sender<Value>>>, ApiErr> {
    ctx.page_replies.lock().map_err(|e| {
        tracing::error!("control: page_replies 锁中毒: {e}");
        ApiErr::new(500, "INTERNAL", "页面回执登记锁异常")
    })
}

/// eval 载荷双次 JSON 序列化：外层即合法 JS 字符串字面量（防注入）。
fn trigger_frontend<R: Runtime>(ctx: &ControlCtx<R>, request: &Value) {
    let payload = serde_json::to_string(request).unwrap_or_else(|_| "{}".to_string());
    let literal = serde_json::to_string(&payload).expect("JSON 字符串再序列化必成功");
    let script = format!("window.__P2P_PAGES__ && window.__P2P_PAGES__.request({literal})");
    match ctx.main_window() {
        Some(window) => {
            if let Err(e) = window.eval(&script) {
                tracing::warn!("control: page eval 失败（仍等待事件回执）: {e}");
            }
        }
        None => tracing::warn!("control: 主窗口不存在，page 请求将等待回执直至超时"),
    }
}

/// 前端回执事件 → 按 requestId 派发到在途请求；无主回执留 warn（可观测）。
pub fn install_reply_listener<R: Runtime>(ctx: &Arc<ControlCtx<R>>) {
    let state = Arc::clone(ctx);
    ctx.app.listen_any(super::PAGE_REPLY_EVENT, move |event| {
        let payload: Value = match serde_json::from_str(event.payload()) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("control: 页面回执载荷非法 JSON: {e}");
                return;
            }
        };
        let Some(request_id) = payload.get("requestId").and_then(Value::as_str) else {
            tracing::warn!("control: 页面回执缺 requestId，丢弃");
            return;
        };
        match state.page_replies.lock() {
            Ok(mut pending) => match pending.remove(request_id) {
                Some(tx) => {
                    let _ = tx.send(payload);
                }
                None => tracing::warn!("control: 页面回执无对应在途请求: {request_id}"),
            },
            Err(e) => tracing::error!("control: page_replies 锁中毒: {e}"),
        }
    });
}
