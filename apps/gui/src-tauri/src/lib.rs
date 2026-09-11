//! Tauri 桥接层：把 p2p::Node 能力按 gui-contract.md §1 命令面 / §2 事件面暴露给前端。
//!
//! 模块划分：types（契约 serde 镜像）/ config（配置持久化）/ proto（echo 与 target 解析）/
//! state（节点生命周期）/ events（事件转发）/ commands（11 个 IPC 命令）/
//! frontend_log（契约 v3 加法：前端错误落盘，G-H 观测）/
//! update（契约 v4 加法：在线更新检查，G-U1）/
//! console（契约 §15：acp 泵进程内装配，INLINE-ACP-PUMP T3）/
//! llm_share（契约 v11 §16 加法：llm-share 命令面，LSG1）。

pub mod a2a;
pub mod acp_descriptor;
pub mod authz;
pub mod chat;
pub mod commands;
pub mod config;
pub mod console;
pub mod control;
pub mod events;
pub mod frontend_log;
pub mod ginvite;
pub mod group;
pub mod history;
pub mod llm_share;
pub mod media_export;
pub mod profile;
pub mod proto;
pub mod state;
pub mod types;
pub mod tunnel;
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
            chat::chat_friend_invite,
            chat::chat_invites_list,
            chat::chat_invite_accept,
            chat::chat_invite_reject,
            chat::chat_invite_cancel,
            chat::chat_friend_update,
            chat::chat_peer_profile,
            chat::chat_friend_remove,
            chat::chat_history,
            chat::chat_send,
            chat::chat_media_file,
            media_export::chat_media_export,
            ginvite::chat_group_invite_send,
            ginvite::chat_group_invites_list,
            ginvite::chat_group_invite_accept,
            ginvite::chat_group_invite_reject,
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
            console::acp_console_status,
            acp_descriptor::acp_local_descriptor,
            llm_share::llm_share_offer_publish,
            llm_share::llm_share_offer_show,
            llm_share::llm_share_allow_list,
            llm_share::llm_share_allow,
            llm_share::llm_share_deny,
            llm_share::llm_share_borrow,
            llm_share::llm_share_ledger_list,
            llm_share::llm_share_ledger_balance,
            llm_share::llm_share_receipt_verify,
            llm_share::llm_share_provider_list,
            llm_share::llm_share_provider_save,
            llm_share::llm_share_provider_remove,
            llm_share::llm_share_share_create,
            llm_share::llm_share_share_list,
            llm_share::llm_share_share_revoke,
            llm_share::llm_share_share_redeem,
            llm_share::llm_share_serve_status,
            a2a::a2a_list,
            a2a::a2a_publish,
            a2a::a2a_unpublish,
            a2a::a2a_allow,
            a2a::a2a_disallow,
            authz::authz_role_list,
            authz::authz_bindings_list,
            authz::authz_bind,
            authz::authz_unbind,
            authz::authz_check,
            authz::authz_default_role_get,
            authz::authz_default_role_save,
            tunnel::tunnel_serve_start,
            tunnel::tunnel_serve_stop,
        ])
        .plugin(tauri_plugin_opener::init())
        // 媒体附件导出（契约 §12 加法）：系统保存对话框选目标路径
        .plugin(tauri_plugin_dialog::init())
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
            // LSG1 llm-share 命令面（契约 §16）：域数据根 = app 数据目录。
            app.manage(llm_share::LlmShareStore::new(dir.clone()));
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
            // acp 泵进程内装配（契约 §15）：装配失败转 disconnected 留痕不阻断
            // 主功能；phase 变更经 acp-console 事件推送；RunEvent::Exit 收尾停泵。
            let console = console::Manager::spawn(dir.clone());
            app.manage(console);
            console::spawn_forwarder(
                app.handle().clone(),
                app.state::<console::Manager>().subscribe(),
            );
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("p2p-console 启动失败");
    app.run(|app, event| {
        if let tauri::RunEvent::Exit = event {
            if let Some(handle) = app.try_state::<control::ControlHandle<tauri::Wry>>() {
                handle.shutdown();
            }
            if let Some(console) = app.try_state::<console::Manager>() {
                console.shutdown();
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
