//! tunnel 被访侧装配（W-T2）：TunnelResponder 注册进 GUI 进程内 Node 的
//! handler 表，会话态默认关闭、不持久化（重启回落关闭）。
//! 命令面（gui-contract §19 冻结签名）：
//! - `tunnel_serve_start(target)`：target 非字面量 `127.0.0.1:<port>` → Err
//!   可读中文且不得部分生效（先校验后动作）；合法则累积白名单 + 开启。
//! - `tunnel_serve_stop()`：关闭服务、保留白名单；已有会话不强杀，收口后
//!   `outcome` 自然落终态（responder 审计）。
//!
//! 前端可见的开 DSH 命令面（tunnel_open_dsh / tunnel_status / 事件）属 W-T3 域。

use std::sync::Arc;

use p2p::Node;
use p2p_tunnel::{TcpDialer, TunnelGate, TunnelResponder, TunnelServeConfig};
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Mutex;

use crate::state::AppState;

/// 持久化白名单恢复进 gate（node_start 装配）：逐条 add_allow，非法条目
/// 丢弃并告警（不静默）；gate 去重语义保证幂等。
fn seed_allowlist(gate: &TunnelGate, persisted: &[String]) {
    for target in persisted {
        if let Err(e) = gate.add_allow(target) {
            tracing::warn!(error = %e, target = %target, "持久化 tunnelServeAllow 条目非法，未恢复");
        }
    }
}

/// serve 受理目标写通持久化（CC2）：去重合并后原子保存；失败 warn 不静默
/// （运行态已生效，重启后该条丢失）。gate.add_allow 先校验后动作，进到这里
/// 的 target 已是合法 loopback 字面量。
fn persist_serve_allow(state: &AppState, target: &str) {
    let mut cfg = state.config_get();
    if cfg.tunnel_serve_allow.iter().any(|t| t == target) {
        return;
    }
    cfg.tunnel_serve_allow.push(target.to_owned());
    if let Err(e) = state.config_save(cfg) {
        tracing::warn!(error = %e, target = %target, "tunnelServeAllow 写盘失败");
    }
}

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

    /// node_start 装配：handler 进表但开关默认关（未显式开启一律拒绝）；
    /// 持久化白名单恢复进 gate 供展示（开关仍关，CC2）。
    pub async fn install(&self, node: &Node, persisted_allow: &[String]) {
        let gate = Arc::new(TunnelGate::new(TunnelServeConfig::default()));
        seed_allowlist(&gate, persisted_allow);
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

/// 显式开启被访侧隧道：target = `127.0.0.1:<port>`（累积加入白名单并写通持久化）。
#[tauri::command]
pub async fn tunnel_serve_start(
    state: State<'_, AppState>,
    target: String,
) -> Result<TunnelServeStatus, String> {
    let status = state.tunnel_serve().start(&target).await?;
    persist_serve_allow(&state, &target);
    Ok(status)
}

/// 关闭被访侧隧道（幂等；保留白名单，新建流回落 shutdown 错误码）。
#[tauri::command]
pub async fn tunnel_serve_stop(state: State<'_, AppState>) -> Result<TunnelServeStatus, String> {
    state.tunnel_serve().stop().await
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 独立临时目录：测试间互不污染，结束清理。
    fn temp_root(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("p2p-tunnel-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("创建临时目录");
        dir
    }

    /// CC2：serve 受理目标写通持久化 + 去重；重启（新 AppState 同目录）可恢复。
    #[test]
    fn serve_allow_persists_deduped_and_survives_restart() {
        let dir = temp_root("allow");
        let state = AppState::new(dir.clone());
        persist_serve_allow(&state, "127.0.0.1:5900");
        persist_serve_allow(&state, "127.0.0.1:5900");
        persist_serve_allow(&state, "127.0.0.1:5901");
        assert_eq!(
            state.config_get().tunnel_serve_allow,
            vec!["127.0.0.1:5900".to_owned(), "127.0.0.1:5901".to_owned()],
            "重复目标只落一条"
        );
        let rebooted = AppState::new(dir);
        assert_eq!(
            rebooted.config_get().tunnel_serve_allow,
            vec!["127.0.0.1:5900".to_owned(), "127.0.0.1:5901".to_owned()],
            "重启后从配置恢复"
        );
    }

    /// 写盘失败路径：不 panic、不假装成功（save 落 warn 日志，可观测）。
    #[test]
    fn persist_failure_warns_without_faking_success() {
        let dir = temp_root("allow-fail");
        let file = dir.join("not-a-dir");
        std::fs::write(&file, "x").expect("占位文件");
        let state = AppState::new(file);
        persist_serve_allow(&state, "127.0.0.1:5900");
        assert!(
            state.config_get().tunnel_serve_allow.is_empty(),
            "保存失败不得落内存假成功"
        );
    }

    /// node_start 装配恢复持久化白名单进 gate；非法条目丢弃；开关仍关。
    #[test]
    fn install_seed_restores_persisted_allow_into_gate() {
        let gate = TunnelGate::new(TunnelServeConfig::default());
        seed_allowlist(&gate, &["127.0.0.1:5900".into(), "10.0.0.1:80".into()]);
        let status = gate.status();
        assert_eq!(
            status.allow,
            vec!["127.0.0.1:5900".to_owned()],
            "非法条目被 gate 拒收"
        );
        assert!(!status.enabled, "恢复仅展示，开关仍回落关闭");
    }
}
