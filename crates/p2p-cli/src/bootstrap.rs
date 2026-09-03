//! bootstrap 子命令：rendezvous（facade 节点自带）+ relay 服务端装配。
//!
//! rendezvous 走 facade Node 的协议分发（/p2p-base/rendezvous/1）；relay 走独立
//! 传输监听（每连接=一条 RelayLink，流为裸 RelayMsg，与 rendezvous 的协议 ID 分帧
//! 互不兼容，故分开监听端口，详见最终报告设计决策）。relay 传输装配抽在
//! relay_serve 模块，与 metrics 子命令共享。

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use p2p::NodeBuilder;
use tokio::sync::broadcast;

use crate::cli::{parse_socket_addr, BootstrapArgs};
use crate::metrics_log::{log_interval, render_relay_metrics};
use crate::relay_serve::{load_identity, spawn_relay};

/// 装配并常驻：facade 节点（rendezvous）+ relay 服务，ctrl-c 优雅退出。
pub async fn run(args: BootstrapArgs) -> Result<(), String> {
    let quic_addr = parse_socket_addr(&args.listen_quic)?;
    let tcp_addr = parse_socket_addr(&args.listen_tcp)?;

    let node = NodeBuilder::new()
        .mdns(false)
        .data_dir(PathBuf::from(&args.data))
        .quic_port(quic_addr.port())
        .tcp_port(tcp_addr.port())
        .observation_responder(args.observation_port)
        .rendezvous_public_only(!args.allow_private)
        .build()
        .await
        .map_err(|e| format!("rendezvous/监听装配失败: {e}"))?;

    let peer = node.local_peer_id();
    let keypair =
        load_identity(PathBuf::from(&args.data)).map_err(|e| format!("读取身份失败: {e}"))?;
    let rendezvous_addrs = node.listen_addrs();
    // relay 用 +3 偏移：+2 的 UDP 口已被观测反射器占用（observation_port）
    let relay_quic = SocketAddr::new(IpAddr::from([0, 0, 0, 0]), quic_addr.port() + 3);
    let relay_tcp = SocketAddr::new(IpAddr::from([0, 0, 0, 0]), tcp_addr.port() + 3);
    let relay = spawn_relay(keypair, relay_quic, relay_tcp)
        .await
        .map_err(|e| format!("relay 监听/装配失败: {e}"))?;

    println!("peer_id={peer}");
    println!(
        "rendezvous (QUIC {} / TCP {})",
        quic_addr.port(),
        tcp_addr.port()
    );
    println!("rendezvous_addrs={rendezvous_addrs:?}");
    println!(
        "relay (QUIC {} / TCP {})",
        relay_quic.port(),
        relay_tcp.port()
    );

    let mut events = node.events();
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
            _ = metrics_tick.tick() => {
                tracing::info!(target: "p2p_metrics", snapshot = ?node.metrics(), "metrics snapshot");
                // E8-M2：relay 指标另以稳定 key=value 打到 stdout（原 Debug 日志保留）
                print!("{}", render_relay_metrics(&relay.metrics()));
                tracing::info!(target: "p2p_metrics", relay = ?relay.metrics(), "relay metrics snapshot");
            }
            ev = events.recv() => match ev {
                Ok(ev) => tracing::debug!(event = ?ev, "bootstrap event"),
                Err(broadcast::error::RecvError::Lagged(skip)) =>
                    tracing::warn!(skip, "event channel lagged"),
                Err(broadcast::error::RecvError::Closed) => {
                    node.shutdown();
                    return Ok(());
                }
            }
        }
    }
}
