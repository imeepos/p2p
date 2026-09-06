//! acp-console 伴生进程托管（gui-contract.md §15 v10，UX 易用性波 UX2）：
//! GUI 启动即定位并 spawn（定位失败转 unavailable 留痕，不阻断主功能，R3 先例）、
//! 异常退出指数退避重启、RunEvent::Exit 收尾；快照经 acp_console_status 命令与
//! acp-console 事件（phase 变更即发）暴露给前端。

mod handle;
pub mod locate;
pub mod spec;
pub mod status;
pub mod supervisor;

#[cfg(test)]
mod tests;

pub use status::{AcpConsolePhase, AcpConsoleStatus};

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::watch;

use self::handle::SupervisorHandle;
use spec::{Limits, SpawnSpec};

/// 事件通道名（契约 §15，独立于 node-event）。
pub const CONSOLE_EVENT: &str = "acp-console";

/// managed 托管句柄：status() 供命令读快照，shutdown() 供 RunEvent::Exit 收尾。
pub struct Manager {
    status: watch::Receiver<AcpConsoleStatus>,
    handle: Option<SupervisorHandle>,
}

impl Manager {
    /// GUI setup 入口：定位成功启动监督；失败转 unavailable 留 lastError（§15）。
    pub fn spawn() -> Self {
        match locate::locate() {
            Ok(bin) => {
                tracing::info!("acp-console 定位成功: {}", bin.display());
                Self::start(bin)
            }
            Err(e) => {
                tracing::error!("acp-console 定位失败（GUI 主功能继续运行）: {e}");
                eprintln!("p2p-console: acp-console 定位失败: {e}");
                Self::unavailable(&e)
            }
        }
    }

    fn start(bin: PathBuf) -> Self {
        let (status_tx, status) = watch::channel(AcpConsoleStatus::initial());
        let handle = handle::start(SpawnSpec::production(bin), Limits::production(), status_tx);
        Self {
            status,
            handle: Some(handle),
        }
    }

    /// 定位失败形态：无监督任务，快照恒为 unavailable。
    pub(crate) fn unavailable(last_error: &str) -> Self {
        let (_, status) = watch::channel(AcpConsoleStatus::unavailable(last_error));
        Self {
            status,
            handle: None,
        }
    }

    /// 托管状态快照（acp_console_status 命令返回值）。
    pub fn status(&self) -> AcpConsoleStatus {
        self.status.borrow().clone()
    }

    /// 订阅状态变更（事件转发任务用）。
    pub(crate) fn subscribe(&self) -> watch::Receiver<AcpConsoleStatus> {
        self.status.clone()
    }

    /// RunEvent::Exit 收尾：停监督并终止子进程（幂等；unavailable 形态为 no-op）。
    pub fn shutdown(&self) {
        if let Some(handle) = &self.handle {
            handle.shutdown();
        }
    }
}

/// acp_console_status（契约 §15 命令表加法）：托管状态快照。
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
