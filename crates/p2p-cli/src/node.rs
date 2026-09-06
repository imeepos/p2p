//! node 子命令：基于 facade 起节点，内置 echo handler，事件打日志，常驻。

use std::path::PathBuf;
use std::sync::Arc;

use p2p::{Node, NodeBuilder, NodeEvent};
use tokio::sync::broadcast;

use crate::cli::{parse_socket_addr, NodeArgs};
use crate::echo::EchoHandler;
use crate::metrics_log::log_interval;
use crate::notice::{NetworkNotice, PublicEndpoint};

/// 常驻运行：注册 echo handler，订阅事件打日志，ctrl-c 时优雅关停。
pub async fn run(args: NodeArgs) -> Result<(), String> {
    let name = args.name.clone().unwrap_or_default();
    let notice = notice_of(&args);
    let node = build_node(&args).await.map_err(|e| e.to_string())?;

    let peer = node.local_peer_id();
    let mut events = node.events();
    println!("[node{name}] peer_id={peer}");
    println!("[node{name}] listen_addrs={:?}", node.listen_addrs());
    // F8：公网外联显式声明（人读形态；装配日志同步留 lan-only 模式信号）
    println!("[node{name}] {}", notice.text());

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
                Ok(NodeEvent::PeerDiscovered { peer, addrs, source }) =>
                    tracing::info!(%peer, ?addrs, ?source, "discovered"),
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
/// lan-only 接线交 facade 装配期统一剥离（公网端点不产生任何外联动作）。
async fn build_node(args: &NodeArgs) -> Result<Node, Box<dyn std::error::Error>> {
    let mut builder = NodeBuilder::new()
        .mdns(!args.no_mdns)
        .data_dir(PathBuf::from(&args.data))
        .lan_only(args.lan_only);
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
    let handler = EchoHandler::new().map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    node.handle_protocol(Arc::new(handler));
    Ok(node)
}

/// 启动声明事实源：lan-only 走单行声明；公网模式按非空参数逐类列出意图。
fn notice_of(args: &NodeArgs) -> NetworkNotice {
    if args.lan_only {
        return NetworkNotice::lan_only();
    }
    let mut endpoints = Vec::new();
    if !args.bootstrap.is_empty() {
        endpoints.push(PublicEndpoint {
            category: "bootstrap",
            purpose: "rendezvous 跨网发现注册与查号",
            addrs: args.bootstrap.clone(),
        });
    }
    if !args.relay.is_empty() {
        endpoints.push(PublicEndpoint {
            category: "relay",
            purpose: "打洞失败后的中继兜底",
            addrs: args.relay.clone(),
        });
    }
    if !args.observation.is_empty() {
        endpoints.push(PublicEndpoint {
            category: "observation",
            purpose: "学习自身公网映射地址",
            addrs: args.observation.clone(),
        });
    }
    NetworkNotice::public(endpoints)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notice_of_lan_only_omits_public_endpoints() {
        let args = NodeArgs {
            data: "d".into(),
            bootstrap: vec!["43.240.223.138/u3400".into()],
            relay: vec!["43.240.223.138/u3403".into()],
            observation: vec!["121.196.193.177:3402".into()],
            lan_only: true,
            name: None,
            listen_quic: None,
            no_mdns: false,
        };
        let notice = notice_of(&args);
        assert!(notice.lan_only);
        assert!(notice.endpoints.is_empty(), "lan-only 不得列公网端点");
        assert!(notice.text().contains("仅局域网"));
        assert!(notice.json()["lanOnly"] == serde_json::json!(true));
    }

    #[test]
    fn notice_of_public_mode_lists_wired_categories() {
        let args = NodeArgs {
            data: "d".into(),
            bootstrap: vec!["43.240.223.138/u3400".into()],
            relay: Vec::new(),
            observation: vec!["121.196.193.177:3402".into()],
            lan_only: false,
            name: None,
            listen_quic: None,
            no_mdns: false,
        };
        let notice = notice_of(&args);
        assert_eq!(notice.endpoints.len(), 2, "未接线的类别不得出现");
        assert_eq!(notice.endpoints[0].category, "bootstrap");
        assert_eq!(notice.endpoints[1].category, "observation");
    }
}
