//! 127.0.0.1 HTTP 循环：端口策略 → token 鉴权 → JSON 分发到 handlers。
//! 每请求串行处理：本机控制面无并发需求，截图/录屏天然互斥。

use std::io::Read;
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::Runtime;

use super::handlers;
use super::ControlCtx;

/// 默认端口；P2P_CONTROL_PORT 显式指定时被占用即报错（不静默换口）。
pub const DEFAULT_PORT: u16 = 7819;
/// 请求体上限：本机控制面无大载荷需求，防失控。
const MAX_BODY: u64 = 1024 * 1024;

/// 业务错误：HTTP 状态 + 机器码 + 人话（R3 结构化错误）。
pub struct ApiErr {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl ApiErr {
    pub fn new(status: u16, code: &str, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.to_string(),
            message: message.into(),
        }
    }
}

/// 绑定 127.0.0.1（禁止对外网卡）：显式端口失败即 Err；默认端口被占用 warn 后回退临时端口。
pub fn listen() -> Result<(tiny_http::Server, SocketAddr), String> {
    let try_bind = |port: u16| -> Result<(tiny_http::Server, SocketAddr), String> {
        let listener = TcpListener::bind(("127.0.0.1", port))
            .map_err(|e| format!("绑定 127.0.0.1:{port} 失败: {e}"))?;
        let addr = listener
            .local_addr()
            .map_err(|e| format!("读取监听地址失败: {e}"))?;
        let server = tiny_http::Server::from_listener(listener, None)
            .map_err(|e| format!("HTTP 服务初始化失败: {e}"))?;
        Ok((server, addr))
    };
    if let Ok(port) = std::env::var("P2P_CONTROL_PORT") {
        let port: u16 = port
            .parse()
            .map_err(|_| format!("P2P_CONTROL_PORT 非法端口号: {port}"))?;
        return try_bind(port);
    }
    match try_bind(DEFAULT_PORT) {
        Ok(ready) => Ok(ready),
        Err(first) => {
            tracing::warn!("control: 默认端口 {DEFAULT_PORT} 不可用（{first}），回退临时端口");
            try_bind(0)
        }
    }
}

pub fn spawn_thread<R: Runtime>(ctx: Arc<ControlCtx<R>>, server: tiny_http::Server) {
    std::thread::Builder::new()
        .name("gc1-control".to_string())
        .spawn(move || run_loop(ctx, server))
        .expect("control: 服务线程启动失败");
}

fn run_loop<R: Runtime>(ctx: Arc<ControlCtx<R>>, server: tiny_http::Server) {
    while !ctx.stop.load(Ordering::SeqCst) {
        match server.recv() {
            Ok(request) => respond(&ctx, request),
            Err(e) => tracing::error!("control: 接受请求失败: {e}"),
        }
    }
    tracing::info!("control: 服务循环退出");
}

fn respond<R: Runtime>(ctx: &Arc<ControlCtx<R>>, mut request: tiny_http::Request) {
    let (status, payload) = dispatch(ctx, &mut request);
    let body = serde_json::to_string(&payload).unwrap_or_else(|_| {
        r#"{"ok":false,"error":{"code":"INTERNAL","message":"响应序列化失败"}}"#.to_string()
    });
    let header = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
        .expect("静态头合法");
    let response = tiny_http::Response::from_string(body)
        .with_status_code(status)
        .with_header(header);
    if let Err(e) = request.respond(response) {
        tracing::warn!("control: 响应发送失败: {e}");
    }
}

fn dispatch<R: Runtime>(
    ctx: &Arc<ControlCtx<R>>,
    request: &mut tiny_http::Request,
) -> (u16, Value) {
    let method = request.method().as_str().to_string();
    let path = request.url().split('?').next().unwrap_or("/").to_string();
    if !authorized(ctx, request) {
        return rejected(
            401,
            "UNAUTHORIZED",
            "缺少或错误的 token：请携带 Authorization: Bearer <token>（见 GUI 数据目录 control/token）",
        );
    }
    let mut body = Vec::new();
    if let Err(e) = request.as_reader().take(MAX_BODY).read_to_end(&mut body) {
        return rejected(400, "INVALID_REQUEST", format!("读取请求体失败: {e}"));
    }
    let payload: Value = if body.is_empty() {
        Value::Null
    } else {
        match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => return rejected(400, "INVALID_REQUEST", format!("请求体非法 JSON: {e}")),
        }
    };
    let result = match (method.as_str(), path.as_str()) {
        ("GET", "/health") => handlers::health(ctx),
        ("POST", "/screenshot") => handlers::screenshot(ctx, &payload),
        ("POST", "/record/start") => handlers::record_start(ctx, &payload),
        ("POST", "/record/stop") => handlers::record_stop(ctx),
        ("POST", "/navigate") => handlers::navigate(ctx, &payload),
        ("POST", "/invoke") => handlers::invoke(ctx, &payload),
        ("GET", "/page/current") => {
            let request_id =
                query_param(request.url(), "requestId").unwrap_or_else(super::page::new_request_id);
            super::page::page_current(ctx, &request_id)
        }
        ("POST", "/page/action") => super::page::page_action(ctx, &payload),
        (
            _,
            "/health" | "/screenshot" | "/record/start" | "/record/stop" | "/navigate" | "/invoke"
            | "/page/current" | "/page/action",
        ) => Err(ApiErr::new(405, "METHOD_NOT_ALLOWED", "HTTP 方法不允许")),
        _ => Err(ApiErr::new(404, "NOT_FOUND", format!("未知端点: {path}"))),
    };
    match result {
        Ok(data) => (200, json!({ "ok": true, "data": data })),
        Err(e) => (
            e.status,
            json!({ "ok": false, "error": { "code": e.code, "message": e.message } }),
        ),
    }
}

fn rejected(status: u16, code: &str, message: impl Into<String>) -> (u16, Value) {
    (
        status,
        json!({ "ok": false, "error": { "code": code, "message": message.into() } }),
    )
}

/// 取 URL query 参数（page/current 的可选 requestId 关联用，省略则服务端生成）。
fn query_param(url: &str, name: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name).then(|| value.to_string())
    })
}

/// Bearer token 恒时校验；无头/错头一律 401（不区分缺 token 与错 token）。
fn authorized<R: Runtime>(ctx: &Arc<ControlCtx<R>>, request: &tiny_http::Request) -> bool {
    let expected = format!("Bearer {}", ctx.token);
    request.headers().iter().any(|h| {
        h.field.equiv("Authorization")
            && crate::control::paths::constant_time_eq(h.value.as_str(), &expected)
    })
}
