//! Tauri 桥接层：把 p2p::Node 能力按 gui-contract.md §1 命令面 / §2 事件面暴露给前端。
//!
//! 模块划分：types（契约 serde 镜像）/ config（配置持久化）/ proto（echo 与 target 解析）/
//! state（节点生命周期）/ events（事件转发）/ commands（9 个 IPC 命令）。

pub mod commands;
pub mod config;
pub mod events;
pub mod proto;
pub mod state;
pub mod types;
pub mod util;

use tauri::Manager;

use crate::state::AppState;

/// 桌面入口：初始化日志、装配状态与命令表。
pub fn run() {
    init_tracing();
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::node_start,
            commands::node_stop,
            commands::node_status,
            commands::metrics_get,
            commands::config_get,
            commands::config_save,
            commands::peer_dial,
            commands::peer_ping,
            commands::identity_reset,
        ])
        .setup(|app| {
            let dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("定位应用数据目录失败: {e}"))?;
            app.manage(AppState::new(dir));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("p2p-console 启动失败");
}

/// 日志走环境变量 RUST_LOG，默认 info；失败路径全部留可观测信号。
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
