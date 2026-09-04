//! Tauri 桥接层：把 p2p::Node 能力按 gui-contract.md §1 命令面 / §2 事件面暴露给前端。
//!
//! 模块划分：types（契约 serde 镜像）/ config（配置持久化）/ proto（echo 与 target 解析）/
//! state（节点生命周期）/ events（事件转发）/ commands（11 个 IPC 命令）/
//! frontend_log（契约 v3 加法：前端错误落盘，G-H 观测）/
//! update（契约 v4 加法：在线更新检查，G-U1）。

pub mod chat;
pub mod commands;
pub mod config;
pub mod control;
pub mod events;
pub mod frontend_log;
pub mod group;
pub mod history;
pub mod profile;
pub mod proto;
pub mod state;
pub mod types;
pub mod update;
pub mod util;
pub mod watcher;

use tauri::Manager;

use crate::state::AppState;

/// 桌面入口：初始化日志、装配状态与命令表。
pub fn run() {
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::node_start,
            commands::node_stop,
            commands::node_status,
            commands::metrics_get,
            commands::metrics_history,
            commands::config_get,
            commands::config_save,
            commands::profile_get,
            commands::profile_save,
            commands::peer_dial,
            commands::peer_connect,
            commands::peer_disconnect,
            commands::peer_ping,
            commands::identity_reset,
            frontend_log::frontend_log_append,
            frontend_log::frontend_log_tail,
            frontend_log::frontend_log_path,
            frontend_log::frontend_log_clear,
            update::update_check,
            update::update_open_release_page,
            chat::chat_friends_list,
            chat::chat_friend_add,
            chat::chat_friend_update,
            chat::chat_friend_remove,
            chat::chat_history,
            chat::chat_send,
            chat::chat_media_file,
            group::group_create,
            group::group_list,
            group::group_invite,
            group::group_kick,
            group::group_leave,
            group::group_rename,
            group::group_disband,
            group::group_send,
            group::group_history,
            group::group_media_file,
        ])
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let log_dir = app
                .path()
                .app_log_dir()
                .map_err(|e| format!("定位应用日志目录失败: {e}"))?;
            init_logging(&log_dir);
            let dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("定位应用数据目录失败: {e}"))?;
            app.manage(AppState::new(dir.clone()));
            let frontend_log = frontend_log::FrontendLog::new(&log_dir)
                .map_err(|e| format!("初始化前端日志失败: {e}"))?;
            app.manage(frontend_log);
            let checker =
                update::UpdateChecker::new().map_err(|e| format!("初始化更新检查器失败: {e}"))?;
            app.manage(checker);
            // GC1 控制通道：启动失败仅显式告警，不阻塞 GUI 主功能（R3）；
            // 句柄入 managed state，RunEvent::Exit 时收尾（停录屏/摘端点文件）。
            match control::spawn(app.handle(), &dir) {
                Ok(handle) => {
                    app.manage(handle);
                }
                Err(e) => {
                    tracing::error!("控制通道启动失败（GUI 继续运行，CLI 将无法连接）: {e}");
                    eprintln!("p2p-console: 控制通道启动失败: {e}");
                }
            }
            // W1 数据目录监听：CLI 写入实时感知。失败已记结构化日志并发
            // data-watch-status{active:false}（R3 降级可判），GUI 主功能不阻断。
            if let Err(e) = watcher::spawn(app.handle().clone(), &dir) {
                eprintln!("p2p-console: 数据目录监听降级: {e}");
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("p2p-console 启动失败");
    app.run(|app, event| {
        if let tauri::RunEvent::Exit = event {
            if let Some(handle) = app.try_state::<control::ControlHandle<tauri::Wry>>() {
                handle.shutdown();
            }
        }
    });
}

/// p2p-log 统一设施接入（替换自带 tracing_subscriber 初始化）：
/// RUST_LOG 默认 info、文本格式、滚动文件落盘到 app_log_dir/p2p-console.log，
/// 并安装 panic 钩子（写日志且回显 stderr）。落盘失败由设施回退 stderr 留告警。
fn init_logging(log_dir: &std::path::Path) {
    let report = p2p_log::init(p2p_log::LogConfig {
        format: p2p_log::LogFormat::Text,
        file: Some(p2p_log::FileOptions::with_default_caps(
            log_dir,
            "p2p-console.log",
        )),
    });
    if let Some(path) = &report.file_path {
        eprintln!("p2p-console: 日志文件 {}", path.display());
    }
    if let Some(fallback) = &report.fallback {
        eprintln!("p2p-console: {fallback}");
    }
}
