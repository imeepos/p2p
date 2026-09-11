//! tunnel 访侧装配（gui-contract §19，W-T3）：本地回环反代 + 隧道客户端的
//! 进程内托管。状态快照经 `tunnel_status` 命令与 `tunnel_status` 事件暴露给
//! 前端（§19.2：单一形状，`serve` 字段恒带，值待 W-T2 接线）；会话建立/
//! 关闭/出错即发事件，错误不静默。W-T2 的 `p2p-tunnel` crate 落地后：
//! `client.rs` 整体切换为其 `TunnelClient` 导出（wire 语义同源冻结契约）。

// 子模块文件与 visit.rs 同级（tunnel/ 目录），#[path] 指向同级。
// 反代核心（proxy/head/pump）已下沉 p2p-tunnel local_proxy（W-TB），
// 此处直接消费 crate 导出面。
#[path = "audit.rs"]
pub mod audit;
#[path = "types.rs"]
pub mod types;
#[path = "url.rs"]
pub mod url;

use std::sync::Arc;

use p2p::PeerId;
use p2p_tunnel::{LocalProxy, ProxyCtx, TunnelClient, TunnelOpener};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::state::AppState;
use tokio::sync::Mutex;

pub use types::{TunnelOpenReport, TunnelStatusReport};

/// Tauri 事件名（§19.2 冻结）。
pub const TUNNEL_EVENT: &str = "tunnel_status";

/// 生产 StreamFactory：开裸流（协议 ID 首帧由 TunnelClient 唯一写入）。
/// 禁用 Node::new_stream——其内嵌握手会与 TunnelClient 叠加成双帧协议 ID
/// （facade 双写缺陷，2026-09-11 裁决），open_raw_stream 是唯一正确缝。
struct NodeStreamFactory {
    node: Arc<p2p::Node>,
}

#[async_trait::async_trait]
impl p2p_protocol::StreamFactory for NodeStreamFactory {
    async fn open_stream(
        &self,
        peer: &PeerId,
        protocol: &p2p_protocol::ProtocolId,
    ) -> std::io::Result<p2p::BoxedStream> {
        self.node
            .open_raw_stream(*peer, protocol.clone())
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))
    }
}

/// 生产开隧道实现：TunnelClient（协议 ID/票据/ack/泵全在 p2p-tunnel 内）。
struct NodeTunnelOpener {
    client: TunnelClient<NodeStreamFactory>,
    peer: PeerId,
}

#[async_trait::async_trait]
impl TunnelOpener for NodeTunnelOpener {
    async fn open(
        &self,
        uid: &str,
        target: &str,
    ) -> Result<p2p_tunnel::TunnelIo, p2p_tunnel::TunnelError> {
        let mut nonce = [0u8; 16];
        getrandom::getrandom(&mut nonce).map_err(|e| {
            p2p_tunnel::TunnelError::Io(std::io::Error::other(format!("nonce: {e}")))
        })?;
        let nonce = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let ticket = p2p_tunnel::TunnelTicket::new(uid, target, nonce)
            .map_err(|e| p2p_tunnel::TunnelError::Io(std::io::Error::other(e)))?;
        self.client.open(self.peer, &ticket).await
    }
}

/// 运行中的访侧会话：反代任务句柄 + 展示面。
pub struct ActiveSession {
    pub local_addr: String,
    pub target: String,
    proxy: tokio::task::JoinHandle<()>,
    ctx: Arc<ProxyCtx>,
}

/// 访侧托管状态（Tauri managed）。serve 面实时取自 W-T2 槽位（TunnelServeSlot）。
#[derive(Default)]
pub struct TunnelState {
    session: Mutex<Option<ActiveSession>>,
}

impl TunnelState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 快照（tunnel_status 命令返回值，§19.2 TunnelStatusReport）。
    pub async fn status(&self, serve: types::TunnelServeStatus) -> TunnelStatusReport {
        let session = self.session.lock().await;
        match session.as_ref() {
            Some(active) => TunnelStatusReport {
                active: true,
                local_addr: Some(active.local_addr.clone()),
                target: Some(active.target.clone()),
                // §19.3-5 八字段全量映射（crate TunnelAuditRecord → camelCase）。
                sessions: active
                    .ctx
                    .audit_snapshot()
                    .await
                    .into_iter()
                    .map(types::TunnelSessionAudit::from)
                    .collect(),
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
        peer: PeerId,
        opener: Arc<dyn TunnelOpener>,
    ) -> Result<TunnelOpenReport, String> {
        self.stop(app).await;
        let result = self.do_start(&target, peer, opener).await;
        match result {
            Ok((session, open_url)) => {
                let report = TunnelOpenReport {
                    local_addr: session.local_addr.clone(),
                    open_url,
                    token: target.token,
                };
                *self.session.lock().await = Some(session);
                self.emit_now(app).await;
                Ok(report)
            }
            Err(reason) => {
                tracing::error!(%reason, "tunnel 会话开启失败");
                self.emit_now(app).await;
                Err(reason)
            }
        }
    }

    /// 以 W-T2 槽位实况组装快照并广播（§19.2 单一形状）。
    async fn emit_now(&self, app: &AppHandle) {
        let serve = app.state::<AppState>().tunnel_serve().peek().await;
        emit_status(app, &self.status(serve).await);
    }

    async fn do_start(
        &self,
        target: &url::DshTarget,
        peer: PeerId,
        opener: Arc<dyn TunnelOpener>,
    ) -> Result<(ActiveSession, String), String> {
        let bound = LocalProxy::bind(target.port, peer_id_str(&peer), opener)
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
        self.emit_now(app).await;
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
/// （由 tunnel.rs 薄包装为 #[tauri::command]：cli-parity 的命令提取只认
/// 两段路径，三段路径会拆出伪命令 "visit"。）
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
    let peer = parse_peer(&peer_raw)?;
    let node = app.state::<AppState>().running_node().await?;
    let opener = Arc::new(NodeTunnelOpener {
        client: TunnelClient::new(NodeStreamFactory { node }),
        peer,
    });
    let report = state.start(&app, target, peer, opener).await?;
    match app.opener().open_url(&report.open_url, None::<&str>) {
        Ok(()) => {}
        Err(e) => tracing::warn!(error = %e, "系统浏览器打开入口链接失败（链接仍可用）"),
    }
    Ok(report)
}

/// 访侧状态快照（§19.1 tunnel_status；serve 实时取自 W-T2 槽位）。
pub async fn tunnel_status(
    app: AppHandle,
    state: tauri::State<'_, TunnelState>,
) -> Result<TunnelStatusReport, String> {
    let serve = app.state::<AppState>().tunnel_serve().peek().await;
    Ok(state.status(serve).await)
}

fn peer_id_str(peer: &PeerId) -> String {
    peer.to_string()
}

/// PeerId 解析（base58，与 CLI/facade 同规则）。
fn parse_peer(raw: &str) -> Result<PeerId, String> {
    let bytes = bs58::decode(raw)
        .into_vec()
        .map_err(|e| format!("peer 非 base58: {e}"))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("peer 长度非法: {raw}"))?;
    Ok(PeerId::from_bytes(arr))
}
