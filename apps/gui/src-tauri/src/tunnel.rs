//! tunnel 被访侧装配（W-T2）：TunnelResponder 注册进 GUI 进程内 Node 的
//! handler 表。开关持久化（2026-09-13 服务总控波 B2，gui-contract §20.1 行为
//! 变化点）：装配时按 `<data-dir>/services.json` 的 serve.tunnel 条目恢复
//! enabled（无条目缺省关；读失败 fail-safe=关 + 告警），start/stop 翻转后
//! upsert 落盘——重启不再回落关闭。白名单仍为内存会话态（问题 11 双存储
//! 不在本卡范围）。
//! 命令面（gui-contract §19 冻结签名）：
//! - `tunnel_serve_start(target)`：target 非字面量 `127.0.0.1:<port>` → Err
//!   可读中文且不得部分生效（先校验后动作）；合法则累积白名单 + 落盘 + 开启。
//! - `tunnel_serve_stop()`：关闭服务、保留白名单；已有会话不强杀，收口后
//!   `outcome` 自然落终态（responder 审计）。
//!
//! 前端可见的开 DSH 命令面（tunnel_open_dsh / tunnel_status / 事件）属 W-T3 域。

use std::path::Path;
use std::sync::Arc;

use p2p::Node;
use p2p_tunnel::{TcpDialer, TunnelGate, TunnelResponder, TunnelServeConfig};
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Mutex;

use crate::state::AppState;

mod serve_persist;

/// TunnelServeStatus（契约 §19，camelCase）：enabled 默认 false、会话态。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelServeStatus {
    pub enabled: bool,
    pub allow: Vec<String>,
    pub active_sessions: u32,
}

/// tunnel 槽位：与节点同生命周期（node_start 注册 handler、node_stop 摘除）。
#[derive(Default)]
pub struct TunnelServeSlot {
    gate: Mutex<Option<Arc<TunnelGate>>>,
}

impl TunnelServeSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// node_start 装配：handler 进表；enabled 按 services.json serve.tunnel
    /// 条目恢复（§20.1 持久化闸），无条目/读失败一律默认关（fail-safe）。
    pub async fn install(&self, node: &Node, data_dir: &Path) {
        let gate = Arc::new(TunnelGate::new(TunnelServeConfig::default()));
        match p2p_tunnel::protocol_id() {
            Ok(protocol) => {
                node.handle_protocol(Arc::new(TunnelResponder::new(
                    protocol,
                    (*gate).clone(),
                    TcpDialer,
                )));
                match serve_persist::restore_enabled(data_dir) {
                    Ok(true) => {
                        gate.set_enabled(true);
                        tracing::info!(
                            "tunnel responder 已注册并恢复启用（services.json serve.tunnel=on）"
                        );
                    }
                    Ok(false) => {
                        tracing::info!("tunnel responder 已注册（serve.tunnel 默认关闭）");
                    }
                    Err(reason) => {
                        tracing::warn!(reason = %reason, "tunnel responder 已注册；serve.tunnel 状态不可读，按关闭 fail-safe")
                    }
                }
                *self.gate.lock().await = Some(gate);
            }
            Err(e) => tracing::error!(error = %e, "tunnel 协议 ID 非法，未注册（不阻断节点启动）"),
        }
    }

    /// node_stop 卸载。
    pub async fn clear(&self) {
        *self.gate.lock().await = None;
    }

    /// 显式开启：先校验 target 并累积白名单（既有语义），再落盘持久化（失败=
    /// 拒绝开启：防「本次开了、重启即丢」的静默意图丢失），最后开启闸门。
    pub async fn start(&self, target: &str, data_dir: &Path) -> Result<TunnelServeStatus, String> {
        let gate = self.gate().await?;
        gate.add_allow(target)?;
        serve_persist::persist_enabled(data_dir, true, serve_persist::now_unix())?;
        gate.set_enabled(true);
        Ok(self.snapshot(&gate))
    }

    /// 关闭（幂等）：handler 仍在表、白名单保留，闸门关 = 新建流一律拒。
    /// 落盘失败不阻断关闭（安全方向优先），但必须留告警（重启可能恢复启用）。
    pub async fn stop(&self, data_dir: &Path) -> Result<TunnelServeStatus, String> {
        let gate = self.gate().await?;
        if let Err(reason) =
            serve_persist::persist_enabled(data_dir, false, serve_persist::now_unix())
        {
            tracing::warn!(
                reason = %reason,
                "serve.tunnel 关闭落盘失败：本次会话仍关闭，重启后可能按 services.json 恢复启用"
            );
        }
        gate.set_enabled(false);
        Ok(self.snapshot(&gate))
    }

    async fn gate(&self) -> Result<Arc<TunnelGate>, String> {
        self.gate
            .lock()
            .await
            .clone()
            .ok_or_else(|| "节点未运行，tunnel 不可用".into())
    }

    fn snapshot(&self, gate: &TunnelGate) -> TunnelServeStatus {
        let status = gate.status();
        TunnelServeStatus {
            enabled: status.enabled,
            allow: status.allow,
            active_sessions: status.active_sessions as u32,
        }
    }
}

/// 显式开启被访侧隧道：target = `127.0.0.1:<port>`（累积加入白名单）。
#[tauri::command]
pub async fn tunnel_serve_start(
    state: State<'_, AppState>,
    target: String,
) -> Result<TunnelServeStatus, String> {
    let data_dir = state.data_dir().to_path_buf();
    state.tunnel_serve().start(&target, &data_dir).await
}

/// 关闭被访侧隧道（幂等；保留白名单，新建流回落 shutdown 错误码）。
#[tauri::command]
pub async fn tunnel_serve_stop(state: State<'_, AppState>) -> Result<TunnelServeStatus, String> {
    let data_dir = state.data_dir().to_path_buf();
    state.tunnel_serve().stop(&data_dir).await
}

// ---- W-T3 访侧（visit 子模块）：反代 + 开 DSH 命令面 ----
// 命令经此 re-export，保持 §19 命令表 `tunnel::tunnel_open_dsh` 字面。
pub mod visit;
pub use visit::TunnelState;

/// §19.1 tunnel_open_dsh 薄包装：实现居 visit 子模块；包装在此使命令表保持
/// 两段路径（cli-parity 命令提取规则 `X::Y` 对三段路径会拆出伪命令）。
#[tauri::command]
pub async fn tunnel_open_dsh(
    app: tauri::AppHandle,
    state: tauri::State<'_, TunnelState>,
    url: String,
    peer: Option<String>,
) -> Result<visit::TunnelOpenReport, String> {
    visit::tunnel_open_dsh(app, state, url, peer).await
}

/// §19.3-9 tunnel_open 薄包装（通用开隧道，实现居 visit 子模块，理由同上）。
#[tauri::command]
pub async fn tunnel_open(
    app: tauri::AppHandle,
    state: tauri::State<'_, TunnelState>,
    target: String,
    peer: String,
) -> Result<visit::TunnelOpenReport, String> {
    visit::tunnel_open(app, state, target, peer).await
}

/// §19.1 tunnel_status 薄包装（同上）。
#[tauri::command]
pub async fn tunnel_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, TunnelState>,
) -> Result<visit::TunnelStatusReport, String> {
    visit::tunnel_status(app, state).await
}

impl TunnelServeSlot {
    /// tunnel_status 的 serve 字段取数：节点未运行/未装配回落默认关闭态。
    pub async fn peek(&self) -> TunnelServeStatus {
        match self.gate().await {
            Ok(gate) => self.snapshot(&gate),
            Err(_) => TunnelServeStatus::default(),
        }
    }
}
