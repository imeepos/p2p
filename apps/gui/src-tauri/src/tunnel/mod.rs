//! tunnel 访侧装配（冻结契约 §6/§7，W-T3）：本地回环反代 + 隧道客户端的
//! 进程内托管。状态快照经 `tunnel_status` 命令与 `tunnel_status` 事件暴露
//! 给前端；开启/关闭/错误即发事件（相位变更语义，console::Manager 先例）。
//! W-T2 的 `p2p-tunnel` crate 落地后：`client.rs` 整体切换为其 `TunnelClient`
//! 导出，`ticket.rs` 的 PROTOCOL_ID 同步改引（wire 语义同源冻结契约）。

pub mod client;
pub mod head;
pub mod proxy;
pub mod ticket;
pub mod types;
pub mod url;

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::state::AppState;

use crate::tunnel::client::NodeDialer;
use tokio::sync::Mutex;

pub use types::{TunnelOpenResult, TunnelStatus};

/// Tauri 事件名（冻结契约 §7 逐字）。
pub const TUNNEL_EVENT: &str = "tunnel_status";

/// 运行中的访侧会话：反代任务句柄 + 展示面。
pub struct ActiveSession {
    pub local_addr: String,
    pub open_url: String,
    pub token: String,
    pub target: String,
    pub peer: String,
    proxy: tokio::task::JoinHandle<()>,
    ctx: Arc<proxy::ProxyCtx>,
}

/// 访侧托管状态（Tauri managed）。
#[derive(Default)]
pub struct TunnelState {
    session: Mutex<Option<ActiveSession>>,
}

impl TunnelState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 快照（tunnel_status 命令返回值）。
    pub async fn status(&self) -> TunnelStatus {
        let session = self.session.lock().await;
        match session.as_ref() {
            Some(active) => TunnelStatus {
                open: true,
                local_addr: Some(active.local_addr.clone()),
                open_url: Some(active.open_url.clone()),
                target: Some(active.target.clone()),
                peer: Some(active.peer.clone()),
                active_conns: active.ctx.active_conns(),
                last_error: None,
                ..Default::default()
            },
            None => TunnelStatus::default(),
        }
    }

    /// 开启会话（先关旧会话保证幂等）；失败时留 last_error 并发事件。
    pub async fn start(
        &self,
        app: &AppHandle,
        target: url::DshTarget,
        peer: String,
        dialer: Arc<dyn client::TunnelDialer>,
    ) -> Result<TunnelOpenResult, String> {
        self.stop(app).await;
        let result = self.do_start(&target, peer, dialer).await;
        match result {
            Ok((open, status)) => {
                *self.session.lock().await = Some(open);
                emit_status(app, &status);
                Ok(TunnelOpenResult {
                    local_addr: status.local_addr.clone().unwrap_or_default(),
                    open_url: status.open_url.clone().unwrap_or_default(),
                    token: target.token,
                })
            }
            Err(reason) => {
                tracing::error!(%reason, "tunnel 会话开启失败");
                let mut status = self.status().await;
                status.last_error = Some(reason.clone());
                status.open = false;
                emit_status(app, &status);
                Err(reason)
            }
        }
    }

    async fn do_start(
        &self,
        target: &url::DshTarget,
        peer: String,
        dialer: Arc<dyn client::TunnelDialer>,
    ) -> Result<(ActiveSession, TunnelStatus), String> {
        let bound =
            proxy::LocalProxy::bind(target.port, dialer)
                .await
                .map_err(|e| format!("本地绑定失败: {e}"))?;
        let local_addr = bound.local_addr();
        let ctx = Arc::clone(&bound.ctx);
        let proxy_task = tokio::spawn(bound.serve());
        let status = TunnelStatus {
            open: true,
            local_addr: Some(local_addr.to_string()),
            open_url: Some(format!(
                "http://{}/?token={}",
                local_addr,
                urlencode(&target.token)
            )),
            target: Some(format!("127.0.0.1:{}", target.port)),
            peer: Some(peer),
            active_conns: 0,
            last_error: None,
            ..Default::default()
        };
        let session = ActiveSession {
            local_addr: status.local_addr.clone().unwrap_or_default(),
            open_url: status.open_url.clone().unwrap_or_default(),
            token: target.token.clone(),
            target: status.target.clone().unwrap_or_default(),
            peer: status.peer.clone().unwrap_or_default(),
            proxy: proxy_task,
            ctx,
        };
        Ok((session, status))
    }

    /// 关闭会话（幂等）：abort 反代任务；活动连接随后自然排空。
    pub async fn stop(&self, app: &AppHandle) {
        let Some(active) = self.session.lock().await.take() else {
            return;
        };
        active.proxy.abort();
        tracing::info!(local_addr = %active.local_addr, "tunnel 会话关闭");
        let mut status = self.status().await;
        status.active_conns = 0;
        emit_status(app, &status);
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


/// 相位变更即发 tunnel_status 事件（契约 §7）。
pub fn emit_status(app: &AppHandle, status: &TunnelStatus) {
    if let Err(e) = app.emit(TUNNEL_EVENT, status) {
        tracing::warn!(error = %e, "推送 tunnel_status 事件失败");
    }
}

/// token 为 base64url 字符集，直通即可；此处仅做最小转义保证 URL 合法。
fn urlencode(token: &str) -> String {
    let mut out = String::with_capacity(token.len());
    for byte in token.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}







/// 开启 DSH 隧道访侧：解析启动 URL → 绑定 127.0.0.1:0 反代 → 成功即以系统
/// 浏览器打开入口链接（失败不否决命令成功——链接已可用，打开途径不止一个）。
#[tauri::command]
pub async fn tunnel_open_dsh(
    app: AppHandle,
    state: tauri::State<'_, TunnelState>,
    url: String,
    peer: Option<String>,
) -> Result<TunnelOpenResult, String> {
    let target = url::parse_dsh_url(&url)?;
    let peer_raw = peer
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .ok_or("未指定被访节点 peer：请填入 A 机节点 PeerId")?;
    let peer_id = parse_peer(&peer_raw)?;
    let node = app.state::<AppState>().running_node().await?;
    let dialer = std::sync::Arc::new(NodeDialer::new(node, peer_id));
    let result = state.start(&app, target, peer_raw, dialer).await?;
    match app.opener().open_url(&result.open_url, None::<&str>) {
        Ok(()) => {}
        Err(e) => tracing::warn!(error = %e, "系统浏览器打开入口链接失败（链接仍可用）"),
    }
    Ok(result)
}

/// 访侧状态快照（含被访侧三字段占位：W-T2 responder Rust API 落地后接线）。
#[tauri::command]
pub async fn tunnel_status(state: tauri::State<'_, TunnelState>) -> Result<TunnelStatus, String> {
    Ok(state.status().await)
}

/// PeerId 解析（base58 58 字符编码，与 CLI/facade 同规则）。
fn parse_peer(raw: &str) -> Result<p2p::PeerId, String> {
    let bytes = bs58::decode(raw)
        .into_vec()
        .map_err(|e| format!("peer 非 base58: {e}"))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("peer 长度非法: {raw}"))?;
    Ok(p2p::PeerId::from_bytes(arr))
}
