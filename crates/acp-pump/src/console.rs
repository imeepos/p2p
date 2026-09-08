//! CLI 装配面：ConsoleConfig → 完整前台泵（P2P 节点 + 本地 WS + status + 发现）。
//! 就绪信息经 stdout JSON 行发布（{"kind":"ready",...}），ctrl_c 后关停。
//! 二进制（p2pctl acp console 等）只做入参翻译，不复制装配实现。

use std::sync::Arc;

use crate::config::ConsoleConfig;
use crate::discovery::{self, DiscoveryHub};
use crate::out;
use crate::share;
use crate::state::StatusHub;
use crate::status::{StatusDeps, StatusServer};
use crate::ticket::TicketStore;
use crate::token;
use crate::ws::{WsDeps, WsServer};

#[derive(serde::Serialize)]
struct ReadyLine {
    ws: String,
    status: String,
    token: String,
    peer: String,
}

/// 起一套前台泵并阻塞到 ctrl_c。run_console 持有运行期语义：数据目录创建、
/// 手动登记、发现转发、WS/status 就绪发布、share 直拨、信号关停。
pub async fn run_console(cfg: ConsoleConfig) -> Result<(), String> {
    let share_link = cfg.share_link.clone();
    let manual_peers = cfg.manual_peers.clone();
    std::fs::create_dir_all(&cfg.data_dir)
        .map_err(|e| format!("data dir {}: {e}", cfg.data_dir.display()))?;
    let node = Arc::new(
        p2p::Node::builder()
            .mdns(cfg.mdns)
            .bootstrap(cfg.bootstrap.clone())
            .data_dir(cfg.data_dir.join("p2p-identity"))
            .build()
            .await
            .map_err(|e| format!("p2p node build: {e}"))?,
    );
    let hub = Arc::new(StatusHub::new());
    let disc = Arc::new(DiscoveryHub::default());
    let tickets = Arc::new(TicketStore::new(&cfg.data_dir));
    let window = cfg.reattach_window;

    spawn_manual_registration(&node, &disc, &manual_peers);
    tokio::spawn(discovery::forward_events(node.clone(), disc.clone()));

    let local_token = token::new_token();
    // WS 先起：status deps 需要 ws.addr（/connect-share 经本地 WS 复用连接编排）。
    let ws = WsServer::start(
        cfg.ws_port,
        local_token.clone(),
        WsDeps {
            node: node.clone(),
            hub: hub.clone(),
            tickets: tickets.clone(),
            window,
        },
    )
    .await
    .map_err(|e| format!("ws server: {e}"))?;
    let status = StatusServer::start(
        cfg.status_port,
        local_token.clone(),
        StatusDeps {
            hub: hub.clone(),
            discovery: disc.clone(),
            tickets: tickets.clone(),
            window,
            node: node.clone(),
            ws_addr: ws.addr,
            ws_token: local_token.clone(),
        },
    )
    .await
    .map_err(|e| format!("status server: {e}"))?;

    let ws_token = local_token.clone();
    out::event(
        "ready",
        &ReadyLine {
            ws: ws.addr.to_string(),
            status: status.addr.to_string(),
            token: local_token,
            peer: node.local_peer_id().to_string(),
        },
    );
    tracing::info!(ws = %ws.addr, status = %status.addr, "acp-pump ready");

    // 分享链接直拨（acp-share §7）：不阻塞就绪发布；结果经 stdout
    // share-connect/state 事件行可观测，禁止静默。
    if let Some(link) = share_link {
        let node = node.clone();
        let hub = hub.clone();
        let disc = disc.clone();
        let ws_addr = ws.addr;
        tokio::spawn(async move {
            share::connect_via_link(node, hub, disc, ws_addr, ws_token, &link).await;
        });
    }

    tokio::signal::ctrl_c()
        .await
        .map_err(|e| format!("signal: {e}"))?;
    tracing::info!("shutdown by signal");
    node.shutdown();
    Ok(())
}

/// 手动登记不阻塞就绪：rendezvous 查号可能等 bootstrap，失败已在内部留痕。
fn spawn_manual_registration(
    node: &Arc<p2p::Node>,
    disc: &Arc<DiscoveryHub>,
    manual: &[(String, Vec<String>)],
) {
    let node = node.clone();
    let disc = disc.clone();
    let manual = manual.to_vec();
    tokio::spawn(async move {
        discovery::apply_manual(&node, &disc, &manual).await;
    });
}
