//! node 子命令：基于 facade 起节点，内置 echo handler，事件打日志，常驻。

use std::path::PathBuf;
use std::sync::Arc;

use p2p::{Node, NodeBuilder, NodeEvent};
use tokio::sync::broadcast;

use crate::cli::{parse_socket_addr, NodeArgs};
use crate::echo::EchoHandler;

/// 常驻运行：注册 echo handler，订阅事件打日志，ctrl-c 时优雅关停。
pub async fn run(args: NodeArgs) -> Result<(), String> {
    let name = args.name.clone().unwrap_or_default();
    let node = build_node(&args).await.map_err(|e| e.to_string())?;

    let peer = node.local_peer_id();
    let mut events = node.events();
    println!("[node{name}] peer_id={peer}");
    println!("[node{name}] listen_addrs={:?}", node.listen_addrs());

    let mut ctrl_c = Box::pin(tokio::signal::ctrl_c());
    loop {
        tokio::select! {
            _ = &mut ctrl_c => {
                tracing::info!("ctrl-c received, shutting down");
                node.shutdown();
                return Ok(());
            }
            ev = events.recv() => match ev {
                Ok(NodeEvent::PeerDiscovered { peer, addrs }) =>
                    tracing::info!(%peer, ?addrs, "discovered"),
                Ok(NodeEvent::PeerConnected { peer }) =>
                    tracing::info!(%peer, "connected"),
                Ok(NodeEvent::PeerDisconnected { peer }) =>
                    tracing::info!(%peer, "disconnected"),
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

/// 装配 facade 节点：mdns on + 可选 bootstrap + 指定/随机 QUIC 端口 + echo handler。
async fn build_node(args: &NodeArgs) -> Result<Node, Box<dyn std::error::Error>> {
    let mut builder = NodeBuilder::new()
        .mdns(true)
        .data_dir(PathBuf::from(&args.data));
    if let Some(addr) = &args.bootstrap {
        builder = builder.bootstrap(vec![addr.clone()]);
    }
    if let Some(q) = &args.listen_quic {
        // facade 恒绑定 0.0.0.0，IP 仅展示；这里只取端口
        let sa = parse_socket_addr(q).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        builder = builder.quic_port(sa.port());
    }
    let node = builder.build().await?;
    node.handle_protocol(Arc::new(EchoHandler));
    Ok(node)
}
