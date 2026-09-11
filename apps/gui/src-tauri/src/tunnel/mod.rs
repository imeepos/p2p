//! tunnel 访侧装配（gui-contract §19，W-T3）：本地回环反代 + 隧道客户端的
//! 进程内托管。状态快照经 `tunnel_status` 命令与 `tunnel_status` 事件暴露给
//! 前端（§19.2：单一形状，`serve` 字段恒带，值待 W-T2 接线）；会话建立/
//! 关闭/出错即发事件，错误不静默。W-T2 的 `p2p-tunnel` crate 落地后：
//! `client.rs` 整体切换为其 `TunnelClient` 导出（wire 语义同源冻结契约）。

pub mod audit;
pub mod client;
pub mod head;
pub mod proxy;
pub mod ticket;
pub mod pump;
pub mod types;
pub mod url;

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::state::AppState;
use crate::tunnel::client::NodeDialer;
use tokio::sync::Mutex;

pub use types::{TunnelOpenReport, TunnelStatusReport};

/// Tauri 事件名（§19.2 冻结）。
pub const TUNNEL_EVENT: &str = "tunnel_status";

/// 运行中的访侧会话：反代任务句柄 + 展示面。
pub struct ActiveSession {
    pub local_addr: String,
    pub target: String,
    proxy: tokio::task::JoinHandle<()>,
    ctx: Arc<proxy::ProxyCtx>,
}

/// 访侧托管状态（Tauri managed）。
#[derive(Default)]
pub struct TunnelState {
    session: Mutex<Option<ActiveSession>>,
    /// 被访侧服务面（W-T2 接线前恒为默认关闭态）。
    serve: Mutex<types::TunnelServeStatus>,
}

impl TunnelState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 快照（tunnel_status 命令返回值，§19.2 TunnelStatusReport）。
    pub async fn status(&self) -> TunnelStatusReport {
        let session = self.session.lock().await;
        let serve = self.serve.lock().await.clone();
        match session.as_ref() {
            Some(active) => TunnelStatusReport {
                active: true,
                local_addr: Some(active.local_addr.clone()),
                target: Some(active.target.clone()),
                sessions: active.ctx.audit_snapshot().await,
                serve,
            },
            None => {
                let mut idle = TunnelStatusReport::idle();
                idle.serve = serve;
                idle
            }
        }
    }

    /// 开启会话（先关旧会话保证幂等）；失败即发事件（错误不静默）。
    pub async fn start(
        &self,
        app: &AppHandle,
        target: url::DshTarget,
        peer_id: String,
        dialer: Arc<dyn client::TunnelDialer>,
    ) -> Result<TunnelOpenReport, String> {
        self.stop(app).await;
        let result = self.do_start(&target, peer_id, dialer).await;
        match result {
            Ok((session, open_url)) => {
                let status = self.status().await;
                *self.session.lock().await = Some(session);
                emit_status(app, &self.status().await);
                Ok(TunnelOpenReport {
                    local_addr: status.local_addr.clone().unwrap_or_default(),
                    open_url,
                    token: target.token,
                })
            }
            Err(reason) => {
                tracing::error!(%reason, "tunnel 会话开启失败");
                emit_status(app, &self.status().await);
                Err(reason)
            }
        }
    }

    async fn do_start(
        &self,
        target: &url::DshTarget,
        peer_id: String,
        dialer: Arc<dyn client::TunnelDialer>,
    ) -> Result<(ActiveSession, String), String> {
        let bound = proxy::LocalProxy::bind(target.port, peer_id.clone(), dialer)
            .await
            .map_err(|e| format!("本地绑定失败: {e}"))?;
        let local_addr = bound.local_addr();
        let ctx = Arc::clone(&bound.ctx);
        let proxy_task = tokio::spawn(bound.serve());
        // §19.3-4：token 只进 open_url 拼装，不进状态/事件/日志。
        let open_url = audit::open_url(local_addr, &target.token);
        let session = ActiveSession {
            local_addr: local_addr.to_string(),
            target: format!("127.0.0.1:{}", target.port),
            proxy: proxy_task,
            ctx,
        };
        Ok((session, open_url))
    }

    /// 关闭会话（幂等）：abort 反代任务；活动连接随后自然排空。
    pub async fn stop(&self, app: &AppHandle) {
        let Some(active) = self.session.lock().await.take() else {
            return;
        };
        active.proxy.abort();
        tracing::info!(local_addr = %active.local_addr, "tunnel 会话关闭");
        emit_status(app, &self.status().await);
    }

    /// W-T2 接线位：被访侧服务面更新即发事件（§19.2 单一形状）。
    pub async fn set_serve(&self, app: &AppHandle, serve: types::TunnelServeStatus) {
        *self.serve.lock().await = serve;
        emit_status(app, &self.status().await);
    }

    /// 退出收尾（RunEvent::Exit 同步路径）：try_lock 取会话并 abort 反代任务；
    /// 拿不到锁说明有并发开启/关闭在进行，留告警日志（禁止静默丢收尾）。
    pub fn shutdown_sync(&self) {
        let mut slot = match self.session.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                tracing::warn!("tunnel 退出收尾未获锁（并发操作中），跳过");
                return;
            }
        };
        if let Some(active) = slot.take() {
            active.proxy.abort();
            tracing::info!(local_addr = %active.local_addr, "tunnel 会话随应用退出关闭");
        }
    }
}

/// 状态变更即发 tunnel_status 事件（§19.2：任一侧变更均发，终态必发）。
pub fn emit_status(app: &AppHandle, status: &TunnelStatusReport) {
    if let Err(e) = app.emit(TUNNEL_EVENT, status) {
        tracing::warn!(error = %e, "推送 tunnel_status 事件失败");
    }
}

/// 开启 DSH 隧道访侧（§19.1）：解析启动 URL（host 必须字面量 127.0.0.1，
/// 先校验后动作）→ 绑定 127.0.0.1:0 反代 → 成功即以系统浏览器打开入口链接
/// （打开失败不否决命令成功——链接仍可用，可复制重开）。
#[tauri::command]
pub async fn tunnel_open_dsh(
    app: AppHandle,
    state: tauri::State<'_, TunnelState>,
    url: String,
    peer: Option<String>,
) -> Result<TunnelOpenReport, String> {
    let target = url::parse_dsh_url(&url)?;
    let peer_raw = peer
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .ok_or("未指定被访节点 peer：请填入 A 机节点 PeerId")?;
    let peer_id = parse_peer(&peer_raw)?;
    let node = app.state::<AppState>().running_node().await?;
    let dialer = Arc::new(NodeDialer::new(node, peer_id));
    let report = state.start(&app, target, peer_raw, dialer).await?;
    match app.opener().open_url(&report.open_url, None::<&str>) {
        Ok(()) => {}
        Err(e) => tracing::warn!(error = %e, "系统浏览器打开入口链接失败（链接仍可用）"),
    }
    Ok(report)
}

/// 访侧状态快照（§19.1 tunnel_status；含被访侧服务面 serve 字段）。
#[tauri::command]
pub async fn tunnel_status(
    state: tauri::State<'_, TunnelState>,
) -> Result<TunnelStatusReport, String> {
    Ok(state.status().await)
}

/// PeerId 解析（base58，与 CLI/facade 同规则）。
fn parse_peer(raw: &str) -> Result<p2p::PeerId, String> {
    let bytes = bs58::decode(raw)
        .into_vec()
        .map_err(|e| format!("peer 非 base58: {e}"))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("peer 长度非法: {raw}"))?;
    Ok(p2p::PeerId::from_bytes(arr))
}
