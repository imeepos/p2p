//! tunnel 被访侧装配（W-T2）：TunnelResponder 注册进 GUI 进程内 Node 的
//! handler 表，会话态默认关闭、不持久化（重启回落关闭）。
//! 命令面（gui-contract §19 冻结签名）：
//! - `tunnel_serve_start(target)`：target 非字面量 `127.0.0.1:<port>` → Err
//!   可读中文且不得部分生效（先校验后动作）；合法则累积白名单 + 开启。
//! - `tunnel_serve_stop()`：关闭服务、保留白名单；已有会话不强杀，收口后
//!   `outcome` 自然落终态（responder 审计）。
//! 前端可见的开 DSH 命令面（tunnel_open_dsh / tunnel_status / 事件）属 W-T3 域。

use std::sync::Arc;

use p2p::Node;
use p2p_tunnel::{TcpDialer, TunnelGate, TunnelResponder, TunnelServeConfig};
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Mutex;

use crate::state::AppState;

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

    /// node_start 装配：handler 进表但开关默认关（未显式开启一律拒绝）。
    pub async fn install(&self, node: &Node) {
        let gate = Arc::new(TunnelGate::new(TunnelServeConfig::default()));
        match p2p_tunnel::protocol_id() {
            Ok(protocol) => {
                node.handle_protocol(Arc::new(TunnelResponder::new(
                    protocol,
                    (*gate).clone(),
                    TcpDialer,
                )));
                *self.gate.lock().await = Some(gate);
                tracing::info!("tunnel responder 已注册（默认关闭）");
            }
            Err(e) => tracing::error!(error = %e, "tunnel 协议 ID 非法，未注册（不阻断节点启动）"),
        }
    }

    /// node_stop 卸载。
    pub async fn clear(&self) {
        *self.gate.lock().await = None;
    }

    /// 显式开启：先校验 target（不部分生效），再累积白名单并开启。
    pub async fn start(&self, target: &str) -> Result<TunnelServeStatus, String> {
        let gate = self.gate().await?;
        gate.add_allow(target)?;
        gate.set_enabled(true);
        Ok(self.snapshot(&gate))
    }

    /// 关闭（幂等）：handler 仍在表、白名单保留，闸门关 = 新建流一律拒。
    pub async fn stop(&self) -> Result<TunnelServeStatus, String> {
        let gate = self.gate().await?;
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
    state.tunnel_serve().start(&target).await
}

/// 关闭被访侧隧道（幂等；保留白名单，新建流回落 shutdown 错误码）。
#[tauri::command]
pub async fn tunnel_serve_stop(state: State<'_, AppState>) -> Result<TunnelServeStatus, String> {
    state.tunnel_serve().stop().await
}
