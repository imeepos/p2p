//! 原语实现：health / screenshot / record / navigate / invoke。
//! 响应 JSON 形状是 p2pctl（GC2）将消费的契约，权威描述见 docs/design/gui-control-channel.md。

use std::path::PathBuf;
use std::sync::MutexGuard;

use serde_json::{json, Value};
use tauri::Runtime;

use super::capture::{self, CaptureError};
use super::invoke_allow;
use super::record::RecordSession;
use super::server::ApiErr;
use super::{ControlCtx, ROUTES};

pub fn health<R: Runtime>(ctx: &ControlCtx<R>) -> Result<Value, ApiErr> {
    let recording = match ctx.record.lock() {
        Ok(slot) => slot.is_some(),
        Err(e) => {
            tracing::error!("control: record 锁中毒: {e}");
            false
        }
    };
    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "title": window_title(ctx),
        "route": ctx.current_route(),
        "pid": std::process::id(),
        "uptimeMs": ctx.started_at.elapsed().as_millis() as u64,
        "recording": recording,
    }))
}

fn window_title<R: Runtime>(ctx: &ControlCtx<R>) -> String {
    match ctx.main_window() {
        Some(w) => w.title().unwrap_or_else(|_| "p2p-console".to_string()),
        None => "p2p-console".to_string(),
    }
}

pub fn screenshot<R: Runtime>(ctx: &ControlCtx<R>, body: &Value) -> Result<Value, ApiErr> {
    let path = out_path(body, "path")?;
    let frame = ctx.frame.capture().map_err(ApiErr::from)?;
    let png = capture::encode_png(&frame)
        .map_err(|e| ApiErr::new(500, "SAVE_FAILED", format!("PNG 编码失败: {e}")))?;
    capture::ensure_png(&png).map_err(|e| ApiErr::new(500, "SAVE_FAILED", e))?;
    let bytes = capture::write_atomic(&path, &png).map_err(|e| ApiErr::new(500, "SAVE_FAILED", e))?;
    Ok(json!({
        "path": path.display().to_string(),
        "width": frame.width,
        "height": frame.height,
        "bytes": bytes,
    }))
}

pub fn record_start<R: Runtime>(ctx: &ControlCtx<R>, body: &Value) -> Result<Value, ApiErr> {
    let path = out_path(body, "path")?;
    let interval = body
        .get("intervalMs")
        .and_then(Value::as_u64)
        .unwrap_or(500)
        .clamp(200, 5000);
    let mut slot = record_slot(ctx)?;
    if slot.is_some() {
        return Err(ApiErr::new(409, "RECORD_CONFLICT", "已有录屏进行中，先 POST /record/stop"));
    }
    let session = RecordSession::start(ctx.frame.clone(), path.clone(), interval)
        .map_err(|e| ApiErr::new(500, "RECORD_START_FAILED", e))?;
    *slot = Some(session);
    Ok(json!({ "path": path.display().to_string(), "intervalMs": interval }))
}

pub fn record_stop<R: Runtime>(ctx: &ControlCtx<R>) -> Result<Value, ApiErr> {
    let mut slot = record_slot(ctx)?;
    let session = slot
        .take()
        .ok_or_else(|| ApiErr::new(409, "RECORD_NOT_ACTIVE", "没有进行中的录屏"))?;
    let stats = session.finalize();
    if stats.bytes == 0 {
        return Err(ApiErr::new(
            500,
            "RECORD_EMPTY",
            format!("录屏未产出有效文件: {}（frames={}）", stats.path, stats.frames),
        ));
    }
    Ok(json!({
        "path": stats.path,
        "frames": stats.frames,
        "bytes": stats.bytes,
        "truncated": stats.truncated,
    }))
}

pub fn navigate<R: Runtime>(ctx: &ControlCtx<R>, body: &Value) -> Result<Value, ApiErr> {
    let name = body
        .get("route")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiErr::new(400, "INVALID_REQUEST", "缺少 route 字段（路由名）"))?;
    let route = name.trim_start_matches('/');
    if !ROUTES.contains(&route) {
        return Err(ApiErr::new(
            400,
            "INVALID_ROUTE",
            format!("未知路由 \"{name}\"，可用: {}", ROUTES.join("/")),
        ));
    }
    let hash = if route == "dashboard" { "#/".to_string() } else { format!("#/{route}") };
    match ctx.main_window() {
        Some(w) => {
            if let Err(e) = w.eval(format!("window.location.hash = '{hash}'")) {
                tracing::warn!("control: 导航 eval 失败（路由态仍更新）: {e}");
            }
        }
        None => tracing::warn!("control: 主窗口不存在，仅更新路由态"),
    }
    set_route(ctx, route);
    Ok(json!({ "route": route, "path": hash }))
}

pub fn invoke<R: Runtime>(ctx: &ControlCtx<R>, body: &Value) -> Result<Value, ApiErr> {
    let command = body
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiErr::new(400, "INVALID_REQUEST", "缺少 command 字段"))?;
    if !invoke_allow::ALLOWED.contains(&command) {
        return Err(ApiErr::new(
            403,
            "INVOKE_FORBIDDEN",
            format!(
                "命令 \"{command}\" 不在控制通道白名单（只读子集: {}）",
                invoke_allow::ALLOWED.join(", ")
            ),
        ));
    }
    let args = body.get("args").cloned().unwrap_or(Value::Null);
    let app = ctx.app.clone();
    let cmd = command.to_string();
    let result = tauri::async_runtime::block_on(async move { invoke_allow::dispatch(&app, &cmd, &args).await })
        .map_err(|e| ApiErr::new(500, "INVOKE_FAILED", e))?;
    Ok(json!({ "result": result }))
}

fn out_path(body: &Value, field: &str) -> Result<PathBuf, ApiErr> {
    let raw = body
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| ApiErr::new(400, "INVALID_REQUEST", format!("缺少 {field} 字段（绝对路径）")))?;
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(ApiErr::new(400, "INVALID_REQUEST", format!("{field} 必须是绝对路径: {raw}")));
    }
    Ok(path)
}

fn record_slot<R: Runtime>(
    ctx: &ControlCtx<R>,
) -> Result<MutexGuard<'_, Option<RecordSession>>, ApiErr> {
    ctx.record.lock().map_err(|e| {
        tracing::error!("control: record 锁中毒: {e}");
        ApiErr::new(500, "INTERNAL", "录屏状态锁异常")
    })
}

fn set_route<R: Runtime>(ctx: &ControlCtx<R>, route: &str) {
    match ctx.route.lock() {
        Ok(mut g) => *g = route.to_string(),
        Err(e) => tracing::error!("control: route 锁中毒: {e}"),
    }
}

impl From<CaptureError> for ApiErr {
    fn from(e: CaptureError) -> Self {
        let status = match e.code {
            "CAPTURE_PERMISSION_DENIED" => 403,
            "CAPTURE_UNAVAILABLE" => 503,
            _ => 500,
        };
        ApiErr::new(status, e.code, e.message)
    }
}
