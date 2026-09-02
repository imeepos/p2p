//! node 子命令：基于 facade 起节点，内置 echo handler，事件打日志，常驻。

use std::path::PathBuf;
use std::sync::Arc;

use p2p::{Node, NodeBuilder, NodeEvent};
use tokio::sync::broadcast;

use crate::cli::{parse_socket_addr, NodeArgs};
use crate::echo::EchoHandler;
use crate::metrics_log::log_interval;

/// 常驻运行：注册 echo handler，订阅事件打日志，ctrl-c 时优雅关停。
pub async fn run(args: NodeArgs) -> Result<(), String> {
    let name = args.name.clone().unwrap_or_default();
    let node = build_node(&args).await.map_err(|e| e.to_string())?;

    let peer = node.local_peer_id();
    let mut events = node.events();
    println!("[node{name}] peer_id={peer}");
    println!("[node{name}] listen_addrs={:?}", node.listen_addrs());

    let mut metrics_tick = tokio::time::interval(log_interval());
    metrics_tick.tick().await; // 首个 tick 立即返回，跳过避免启动刷屏
    let mut ctrl_c = Box::pin(tokio::signal::ctrl_c());
    loop {
        tokio::select! {
            _ = &mut ctrl_c => {
                tracing::info!("ctrl-c received, shutting down");
                node.shutdown();
                return Ok(());
            }
            _ = metrics_tick.tick() =>
                tracing::info!(target: "p2p_metrics", snapshot = ?node.metrics(), "metrics snapshot"),
            ev = events.recv() => match ev {
                Ok(NodeEvent::PeerDiscovered { peer, addrs }) =>
                    tracing::info!(%peer, ?addrs, "discovered"),
                Ok(NodeEvent::PeerConnected { peer }) =>
                    tracing::info!(%peer, "connected"),
                Ok(NodeEvent::PeerDisconnected { peer }) =>
                    tracing::info!(%peer, "disconnected"),
                Ok(NodeEvent::DialHop { peer, hop, ok, detail }) =>
                    tracing::info!(%peer, ?hop, ok, %detail, "dial hop"),
                Ok(other) => tracing::debug!(event = ?other, "event"),
                Err(broadcast::error::RecvError::Lagged(skip)) =>
                    tracing::warn!(skip, "event channel lagged, dropped events"),
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::warn!("event channel closed; node likely shut down");
                    node.shutdown();
                    return Ok(());
                }
            },
        }
    }
}

/// 装配 facade 节点：mdns on + 可选 bootstrap/relay + 指定/随机 QUIC 端口 + echo handler。
async fn build_node(args: &NodeArgs) -> Result<Node, Box<dyn std::error::Error>> {
    let mut builder = NodeBuilder::new()
        .mdns(!args.no_mdns)
        .data_dir(PathBuf::from(&args.data));
    if !args.bootstrap.is_empty() {
        builder = builder.bootstrap(args.bootstrap.clone());
    }
    if !args.relay.is_empty() {
        builder = builder.relay_addrs(args.relay.clone());
    }
    if let Some(q) = &args.listen_quic {
        // facade 恒绑定 0.0.0.0，IP 仅展示；这里只取端口
        let sa = parse_socket_addr(q).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        builder = builder.quic_port(sa.port());
    }
    if !args.observation.is_empty() {
        builder = builder.observation_addrs(args.observation.clone());
    }
    let node = builder.build().await?;
    node.handle_protocol(Arc::new(EchoHandler));
    Ok(node)
}
