//! gui-contract.md §1 的 11 个 Tauri 命令：薄封装，业务在 state，事件在 events。
//!
//! 参数名经 Tauri 自动转 camelCase（peerId/timeoutMs），与契约逐字对齐；Err 一律中文。

use tauri::{AppHandle, Runtime, State};

use crate::events;
use crate::profile::NodeProfile;
use crate::state::AppState;
use crate::history::MetricsPoint;
use crate::types::{DialReport, GuiConfig, MetricsJson, NodeEventJson, NodeStatus, PingOutcome};

/// node_start：构建 Node 并启动；已运行 Err。
/// 事件订阅先于返回（state.start 内建立），node_started 由桥接层自产。
#[tauri::command]
pub async fn node_start<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    cfg: GuiConfig,
) -> Result<NodeStatus, String> {
    let started = state.start(cfg).await?;
    events::spawn(app.clone(), started.events);
    events::emit(
        &app,
        NodeEventJson::NodeStarted {
            listen_addrs: started.listen_addrs,
            ts_ms: None,
        },
    );
    Ok(started.status)
}

/// node_stop：幂等；node_stopped 仅在真的停掉运行中节点时发出。
#[tauri::command]
pub async fn node_stop<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<NodeStatus, String> {
    let stopped = state.stop().await;
    if stopped {
        events::emit(&app, NodeEventJson::NodeStopped { ts_ms: None });
    }
    Ok(state.status().await)
}

/// node_status：本地/监听地址/运行时长快照。
#[tauri::command]
pub async fn node_status(state: State<'_, AppState>) -> Result<NodeStatus, String> {
    Ok(state.status().await)
}

/// metrics_get：运行时指标；未运行返回全零。
#[tauri::command]
pub async fn metrics_get(state: State<'_, AppState>) -> Result<MetricsJson, String> {
    Ok(state.metrics().await)
}

/// config_get：读持久化配置，无文件返回默认值。
#[tauri::command]
pub async fn config_get(state: State<'_, AppState>) -> Result<GuiConfig, String> {
    Ok(state.config_get())
}

/// config_save：原子写盘；不改变运行中节点。
#[tauri::command]
pub async fn config_save(state: State<'_, AppState>, cfg: GuiConfig) -> Result<GuiConfig, String> {
    state.config_save(cfg)
}

/// profile_get：读持久化节点资料，无文件返回默认值（契约 v6 §11）。
#[tauri::command]
pub async fn profile_get(state: State<'_, AppState>) -> Result<NodeProfile, String> {
    Ok(state.profile_get())
}

/// profile_save：校验（长度/头像格式）后原子写盘；不改变运行中节点，无需重启即生效。
#[tauri::command]
pub async fn profile_save(
    state: State<'_, AppState>,
    profile: NodeProfile,
) -> Result<NodeProfile, String> {
    state.profile_save(profile)
}

/// peer_dial：target 形如 "<peer_id>@<addr>"（契约 §6），逐跳报告随 DialReport 回收。
#[tauri::command]
pub async fn peer_dial(state: State<'_, AppState>, target: String) -> Result<DialReport, String> {
    state.dial(&target).await
}

/// peer_connect：按地址簿连接已知节点（节点页行内拨号），逐跳随 DialReport 回收。
#[tauri::command]
pub async fn peer_connect(
    state: State<'_, AppState>,
    peer_id: String,
) -> Result<DialReport, String> {
    state.connect(&peer_id).await
}

/// peer_disconnect：挂断与该节点的连接；幂等，返回是否确有连接被关闭。
#[tauri::command]
pub async fn peer_disconnect(
    state: State<'_, AppState>,
    peer_id: String,
) -> Result<bool, String> {
    state.disconnect(&peer_id).await
}

/// peer_ping：复用 echo 协议 request（同 CLI ping），返回 rtt 与期间逐跳。
#[tauri::command]
pub async fn peer_ping(
    state: State<'_, AppState>,
    peer_id: String,
    timeout_ms: u64,
) -> Result<PingOutcome, String> {
    state.ping(&peer_id, timeout_ms).await
}

/// identity_reset：危险操作，confirm 必须为 true；停节点并删除身份种子文件。
#[tauri::command]
pub async fn identity_reset<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    confirm: bool,
) -> Result<NodeStatus, String> {
    if !confirm {
        return Err("重置身份是危险操作，必须显式传入 confirm=true".into());
    }
    let (status, was_running) = state.reset_identity().await?;
    if was_running {
        events::emit(&app, NodeEventJson::NodeStopped { ts_ms: None });
    }
    Ok(status)
}
/// metrics_history：运行期 5s 采样的最近 120 点；未运行返回空数组（契约 v2）。
#[tauri::command]
pub async fn metrics_history(state: State<'_, AppState>) -> Result<Vec<MetricsPoint>, String> {
    Ok(state.metrics_history().await)
}

