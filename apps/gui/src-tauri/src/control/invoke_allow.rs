//! 控制通道 invoke 显式白名单：generate_handler! 的只读子集（显式枚举，非反射面）。
//! 红线：只收只读命令；写操作（start/stop/save/dial/reset…）永不入列。

use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};

use crate::state::AppState;

/// 与 lib.rs generate_handler! 对齐的显式只读子集。
pub const ALLOWED: &[&str] = &[
    "node_status",
    "metrics_get",
    "metrics_history",
    "config_get",
    "profile_get",
];

/// 白名单命令已由 handler 拦截，此处 match 是第二道防线（防御未知名字）。
pub async fn dispatch<R: Runtime>(
    app: &AppHandle<R>,
    command: &str,
    args: &Value,
) -> Result<Value, String> {
    if args.as_object().is_some_and(|m| !m.is_empty()) {
        tracing::warn!("control: invoke {command} 忽略非空 args（白名单命令均无参数）");
    }
    let state = app.state::<AppState>();
    match command {
        "node_status" => to_json(state.status().await),
        "metrics_get" => to_json(state.metrics().await),
        "metrics_history" => to_json(state.metrics_history().await),
        "config_get" => to_json(state.config_get()),
        "profile_get" => to_json(state.profile_get()),
        _ => Err(format!("命令 {command} 不在白名单")),
    }
}

fn to_json<T: serde::Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| format!("命令结果序列化失败: {e}"))
}
