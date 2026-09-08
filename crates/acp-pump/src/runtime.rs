//! 泵运行时句柄：Pump::start 装配完整泵（P2P 节点 + 本地 WS + status + 发现）并
//! 立即返回句柄，供 GUI 进程内装配等宿主观察端口/token 与退出原因。退出经
//! catch_unwind 收口：panic/异常一律转 PumpExit::failed 显式失败态，禁止静默。

use std::net::SocketAddr;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use futures_util::FutureExt;
use tokio::sync::watch;

use crate::config::ConsoleConfig;
use crate::discovery::{self, DiscoveryHub};
use crate::state::StatusHub;
use crate::status::{StatusDeps, StatusServer};
use crate::ticket::TicketStore;
use crate::token;
use crate::ws::{WsDeps, WsServer};

/// 泵任务退出原因。ok=false = panic 或运行期错误（error 携带可读原因）。
#[derive(Clone, Debug)]
pub struct PumpExit {
    pub ok: bool,
    pub error: Option<String>,
}

/// Pump::start 返回的句柄：连接面（端口/token/peer）、状态 hub 与退出观察。
/// stop() 幂等；exit watch 恰好投递一次 Some(PumpExit)。
pub struct PumpHandle {
    /// 本地 WS 服务地址（127.0.0.1 绑定）。
    pub ws_addr: SocketAddr,
    /// status HTTP 服务地址（127.0.0.1 绑定）。
    pub status_addr: SocketAddr,
    /// console WS / status 共用鉴权 token。
    pub token: String,
    /// 本节点 PeerId（base58）。
    pub peer: String,
    /// 连接状态机 hub（宿主可订阅 per-peer 连接迁移）。
    pub hub: Arc<StatusHub>,
    exit: watch::Receiver<Option<PumpExit>>,
    stop: watch::Sender<bool>,
}

/// 泵装配入口。装配期错误（目录不可建/节点构建失败/端口绑定失败）经 Err 返回；
/// 就绪后宿主经句柄观察运行期。
pub struct Pump;

impl Pump {
    pub async fn start(cfg: ConsoleConfig) -> Result<PumpHandle, String> {
        std::fs::create_dir_all(&cfg.data_dir)
            .map_err(|e| format!("data dir {}: {e}", cfg.data_dir.display()))?;
        let node = build_node(&cfg).await?;
        let hub = Arc::new(StatusHub::new());
        let disc = Arc::new(DiscoveryHub::default());
        let tickets = Arc::new(TicketStore::new(&cfg.data_dir));
        spawn_manual_registration(&node, &disc, &cfg.manual_peers);
        tokio::spawn(discovery::forward_events(node.clone(), disc.clone()));
        let local_token = token::new_token();
        let (ws_addr, status_addr) =
            start_servers(&node, &hub, &disc, &tickets, &cfg, &local_token).await?;
        // 分享链接直拨（acp-share §7）：不阻塞就绪；结果经 stdout share-connect
        // 事件行与 /status 状态面可观测，禁止静默。
        if let Some(link) = cfg.share_link.clone() {
            let (node, hub, disc, ws_token) =
                (node.clone(), hub.clone(), disc.clone(), local_token.clone());
            tokio::spawn(async move {
                crate::share::connect_via_link(node, hub, disc, ws_addr, ws_token, &link).await;
            });
        }
        let peer = node.local_peer_id().to_string();
        let (stop_tx, stop_rx) = watch::channel(false);
        let (exit_tx, exit_rx) = watch::channel(None);
        spawn_pump_task(node, stop_rx, exit_tx);
        Ok(PumpHandle {
            ws_addr,
            status_addr,
            token: local_token,
            peer,
            hub,
            exit: exit_rx,
            stop: stop_tx,
        })
    }
}

impl PumpHandle {
    /// 请求收尾并等待退出原因（幂等：已退出时直接返回既有原因）。
    pub async fn stop(&self) -> Option<PumpExit> {
        let _ = self.stop.send(true);
        let mut rx = self.exit.clone();
        if let Some(exit) = rx.borrow().clone() {
            return Some(exit);
        }
        match rx.changed().await {
            Ok(()) => rx.borrow().clone(),
            Err(_) => None,
        }
    }

    /// 等待泵任务退出（宿主监听 JoinHandle 的等价物；panic 已收口为 failed）。
    pub async fn wait_exit(&mut self) -> PumpExit {
        loop {
            if let Some(exit) = self.exit.borrow().clone() {
                return exit;
            }
            if self.exit.changed().await.is_err() {
                return PumpExit {
                    ok: false,
                    error: Some("pump exit channel closed".to_string()),
                };
            }
        }
    }

    /// 非阻塞查看退出原因（None = 仍在运行）。
    pub fn try_exit(&self) -> Option<PumpExit> {
        self.exit.borrow().clone()
    }
}

async fn build_node(cfg: &ConsoleConfig) -> Result<Arc<p2p::Node>, String> {
    let node = p2p::Node::builder()
        .mdns(cfg.mdns)
        .bootstrap(cfg.bootstrap.clone())
        .data_dir(cfg.data_dir.join("p2p-identity"))
        .build()
        .await
        .map_err(|e| format!("p2p node build: {e}"))?;
    Ok(Arc::new(node))
}

/// 起本地 WS + status（WS 先起：status deps 需要 ws.addr 复用连接编排）。
async fn start_servers(
    node: &Arc<p2p::Node>,
    hub: &Arc<StatusHub>,
    disc: &Arc<DiscoveryHub>,
    tickets: &Arc<TicketStore>,
    cfg: &ConsoleConfig,
    token: &str,
) -> Result<(SocketAddr, SocketAddr), String> {
    let ws = WsServer::start(
        cfg.ws_port,
        token.to_string(),
        WsDeps {
            node: node.clone(),
            hub: hub.clone(),
            tickets: tickets.clone(),
            window: cfg.reattach_window,
        },
    )
    .await
    .map_err(|e| format!("ws server: {e}"))?;
    let status = StatusServer::start(
        cfg.status_port,
        token.to_string(),
        StatusDeps {
            hub: hub.clone(),
            discovery: disc.clone(),
            tickets: tickets.clone(),
            window: cfg.reattach_window,
            node: node.clone(),
            ws_addr: ws.addr,
            ws_token: token.to_string(),
        },
    )
    .await
    .map_err(|e| format!("status server: {e}"))?;
    Ok((ws.addr, status.addr))
}

/// 手动登记不阻塞就绪：rendezvous 查号可能等 bootstrap，失败已在内部留痕。
fn spawn_manual_registration(
    node: &Arc<p2p::Node>,
    disc: &Arc<DiscoveryHub>,
    manual: &[(String, Vec<String>)],
) {
    let (node, disc) = (node.clone(), disc.clone());
    let manual = manual.to_vec();
    tokio::spawn(async move {
        discovery::apply_manual(&node, &disc, &manual).await;
    });
}

/// 泵主体：只等停机位；收尾即关停节点。
async fn pump_main(node: Arc<p2p::Node>, mut stop: watch::Receiver<bool>) -> Result<(), String> {
    while !*stop.borrow() {
        stop.changed()
            .await
            .map_err(|_| "pump stop channel closed".to_string())?;
    }
    tracing::info!("pump shutdown by stop signal");
    node.shutdown();
    Ok(())
}

/// 泵任务包装：panic 一律收口为 PumpExit::failed（catch_unwind），
/// 结果写退出 watch（恰好一次 Some）。
fn spawn_pump_task(
    node: Arc<p2p::Node>,
    stop: watch::Receiver<bool>,
    exit_tx: watch::Sender<Option<PumpExit>>,
) {
    tokio::spawn(async move {
        let exit = match AssertUnwindSafe(pump_main(node, stop)).catch_unwind().await {
            Ok(Ok(())) => PumpExit {
                ok: true,
                error: None,
            },
            Ok(Err(e)) => PumpExit {
                ok: false,
                error: Some(e),
            },
            Err(payload) => PumpExit {
                ok: false,
                error: Some(panic_text(payload)),
            },
        };
        if let Some(error) = &exit.error {
            tracing::error!(error = %error, "pump task exited abnormally");
        }
        let _ = exit_tx.send(Some(exit));
    });
}

/// panic 载荷转可读文本（未知载荷不静默，给固定标记）。
fn panic_text(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        return format!("panic: {s}");
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return format!("panic: {s}");
    }
    "panic: (non-string payload)".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_payload_converts_to_read_text() {
        let boxed: Box<dyn std::any::Any + Send> = Box::new("boom");
        assert_eq!(panic_text(boxed), "panic: boom");
        let boxed: Box<dyn std::any::Any + Send> = Box::new(7u32);
        assert!(panic_text(boxed).contains("non-string"));
    }

    /// 回环：start 返回连接面，两端口可连，stop 幂等且退出原因 ok。
    #[tokio::test]
    async fn start_binds_servers_and_stop_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("acp-pump-rt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let handle = Pump::start(ConsoleConfig {
            data_dir: dir.clone(),
            ..ConsoleConfig::default()
        })
        .await
        .expect("pump start");
        assert!(handle.ws_addr.port() > 0 && handle.status_addr.port() > 0);
        assert!(!handle.peer.is_empty());
        let first = tokio::net::TcpStream::connect(handle.ws_addr)
            .await
            .expect("ws port listening");
        drop(first);
        let exit = handle.stop().await.expect("first stop yields exit");
        assert!(exit.ok, "正常收尾 ok=true: {exit:?}");
        let again = handle.stop().await;
        assert_eq!(again.map(|e| e.ok), Some(true), "stop 幂等");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
