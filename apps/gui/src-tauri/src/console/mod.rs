//! acp 泵进程内装配（gui-contract.md §15，INLINE-ACP-PUMP T3）：
//! GUI 启动即 Pump::start（失败转 disconnected 留痕，不阻断主功能，R3 先例），
//! 无外部进程与监督重启语义；快照经 acp_console_status 命令与 acp-console
//! 事件（phase 变更即发）暴露给前端。泵任务 panic/异常由 acp-pump 收口为
//! PumpExit::failed，此处一律转 disconnected + lastError 显式失败态，禁止静默。

mod types;

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::{watch, Notify};

use acp_pump::Pump;

pub use types::{AcpConsolePhase, AcpConsoleStatus};

#[cfg(test)]
mod tests;

/// 事件通道名（契约 §15，独立于 node-event；名沿旧 sidecar 波保留）。
pub const CONSOLE_EVENT: &str = "acp-console";
/// pump 数据目录：沿旧 sidecar 约定 app_data_dir/acp-console-data（前端可见路径不回归）。
const DATA_DIR_NAME: &str = "acp-console-data";

/// managed 托管句柄：status() 供命令读快照，shutdown() 供 RunEvent::Exit 收尾。
pub struct Manager {
    status: watch::Receiver<AcpConsoleStatus>,
    stop: Arc<Notify>,
}

impl Manager {
    /// GUI setup 入口：装配驱动任务接手状态迁移；失败只留痕不阻断主功能。
    pub fn spawn(app_data_dir: PathBuf) -> Self {
        let (status_tx, status) = watch::channel(AcpConsoleStatus::connecting());
        let stop = Arc::new(Notify::new());
        tauri::async_runtime::spawn(drive_pump(app_data_dir, Arc::clone(&stop), status_tx));
        Self { status, stop }
    }

    /// 托管状态快照（acp_console_status 命令返回值）。
    pub fn status(&self) -> AcpConsoleStatus {
        self.status.borrow().clone()
    }

    /// 订阅状态变更（事件转发任务用）。
    pub(crate) fn subscribe(&self) -> watch::Receiver<AcpConsoleStatus> {
        self.status.clone()
    }

    /// RunEvent::Exit 收尾（幂等）：请求停泵；状态迁移由驱动任务完成。
    pub fn shutdown(&self) {
        self.stop.notify_waiters();
    }
}

/// 装配驱动任务：start 成功 → connected（连接面来自进程内句柄）；
/// 装配失败或泵退出（含 panic 收口）→ disconnected + lastError。
async fn drive_pump(
    app_data_dir: PathBuf,
    stop: Arc<Notify>,
    status_tx: watch::Sender<AcpConsoleStatus>,
) {
    let cfg = acp_pump::ConsoleConfig {
        data_dir: app_data_dir.join(DATA_DIR_NAME),
        ..acp_pump::ConsoleConfig::default()
    };
    match Pump::start(cfg).await {
        Ok(mut handle) => {
            status_tx.send_modify(|s| *s = AcpConsoleStatus::connected(&handle));
            let exit = wait_exit_or_stop(&mut handle, &stop).await;
            status_tx.send_modify(|s| *s = exit_to_status(&exit));
        }
        Err(err) => {
            tracing::error!(error = %err, "acp pump 装配失败（GUI 主功能继续运行）");
            status_tx.send_modify(|s| *s = AcpConsoleStatus::disconnected(Some(err)));
        }
    }
}

/// 等停机请求或泵退出；停机路径先 stop 拿正常退出原因。
async fn wait_exit_or_stop(handle: &mut acp_pump::PumpHandle, stop: &Notify) -> acp_pump::PumpExit {
    tokio::select! {
        _ = stop.notified() => handle.stop().await.unwrap_or(acp_pump::PumpExit {
            ok: false,
            error: Some("pump stop without exit cause".into()),
        }),
        exit = handle.wait_exit() => exit,
    }
}

/// 退出原因 → 状态面：干净收尾 disconnected 无错；失败态保留 lastError 并留痕。
fn exit_to_status(exit: &acp_pump::PumpExit) -> AcpConsoleStatus {
    if exit.ok {
        return AcpConsoleStatus::disconnected(None);
    }
    let reason = exit
        .error
        .clone()
        .unwrap_or_else(|| "pump exited without reason".to_string());
    tracing::error!(reason = %reason, "acp pump 异常退出");
    AcpConsoleStatus::disconnected(Some(reason))
}

/// acp_console_status（契约 §15 命令名沿旧波保留，语义 = in-process pump 状态）。
#[tauri::command]
pub async fn acp_console_status(
    manager: tauri::State<'_, Manager>,
) -> Result<AcpConsoleStatus, String> {
    Ok(manager.status())
}

/// phase 变更即发 acp-console 事件（§15）；启动快照先发一次，避免订阅窗口丢态。
pub fn spawn_forwarder<R: Runtime>(app: AppHandle<R>, mut rx: watch::Receiver<AcpConsoleStatus>) {
    tauri::async_runtime::spawn(async move {
        let mut last_phase = None;
        loop {
            let status = rx.borrow().clone();
            if last_phase != Some(status.phase) {
                last_phase = Some(status.phase);
                emit_status(&app, &status);
            }
            if rx.changed().await.is_err() {
                tracing::info!("acp-console 状态通道关闭，转发任务退出");
                return;
            }
        }
    });
}

/// 载荷 = AcpConsoleStatus + 可选 tsMs（§2 同款约定：发射时刻毫秒戳）。
fn event_payload(status: &AcpConsoleStatus) -> Option<serde_json::Value> {
    match serde_json::to_value(status) {
        Ok(serde_json::Value::Object(mut map)) => {
            map.insert("tsMs".into(), serde_json::json!(crate::util::now_ms()));
            Some(serde_json::Value::Object(map))
        }
        Ok(other) => {
            tracing::error!("acp-console 状态序列化形状异常: {other}");
            None
        }
        Err(e) => {
            tracing::error!("acp-console 状态序列化失败: {e}");
            None
        }
    }
}

fn emit_status<R: Runtime>(app: &AppHandle<R>, status: &AcpConsoleStatus) {
    let Some(payload) = event_payload(status) else {
        return;
    };
    if let Err(e) = app.emit(CONSOLE_EVENT, &payload) {
        tracing::warn!(error = %e, "推送 acp-console 事件失败");
    }
}
