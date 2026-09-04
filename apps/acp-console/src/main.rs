//! acp-console 入口：装配 P2P 节点、本地 WS 服务、status 端点与发现转发，
//! 就绪信息经 stdout JSON 行发布（{"kind":"ready",...}），等 ctrl_c 后关停。

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use serde::Serialize;

use acp_console::config::{parse_manual_peers, ConsoleConfig};
use acp_console::discovery::{self, DiscoveryHub};
use acp_console::out;
use acp_console::state::StatusHub;
use acp_console::status::{StatusDeps, StatusServer};
use acp_console::ticket::TicketStore;
use acp_console::token;
use acp_console::ws::{WsDeps, WsServer};

#[derive(Parser)]
#[command(
    name = "acp-console",
    about = "ACP over P2P 操作者侧伴生进程：本地 WS(127.0.0.1+token) ⇄ P2P 流哑泵 + 节点发现"
)]
struct Args {
    /// 数据目录：reattach 票据落这里，P2P 身份目录 = <dir>/p2p-identity
    #[arg(long, default_value = "./acp-console-data", value_name = "DIR")]
    data_dir: PathBuf,
    /// rendezvous bootstrap 地址（ip/u端口 或 ip/t端口），可多次
    #[arg(long, value_name = "ADDR")]
    bootstrap: Vec<String>,
    /// 关闭 mDNS 局域网发现
    #[arg(long)]
    no_mdns: bool,
    /// 手动登记候选 PEER@ADDR（base58 PeerId @ 底座传输地址），可多次
    #[arg(long = "peer", value_name = "PEER@ADDR")]
    peers: Vec<String>,
    /// 透传给 agent 桥的握手 token（也可由 WS 查询参数 atoken 逐连接指定）
    #[arg(long, value_name = "TOKEN")]
    agent_token: Option<String>,
    /// 本地 WS 端口（0 = 随机）
    #[arg(long, default_value_t = 0, value_name = "PORT")]
    ws_port: u16,
    /// status HTTP 端口（0 = 随机）
    #[arg(long, default_value_t = 0, value_name = "PORT")]
    status_port: u16,
    /// 断流续连窗口秒数（设计 §5 默认 90）
    #[arg(long, default_value_t = acp_common::consts::REATTACH_WINDOW_DEFAULT_SECS, value_name = "SECS")]
    window_secs: u64,
}

#[derive(Serialize)]
struct ReadyLine {
    ws: String,
    status: String,
    token: String,
    peer: String,
}

fn main() -> Result<(), String> {
    let args = Args::parse();
    let _ = p2p_log::init(p2p_log::LogConfig::default());
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("tokio runtime: {e}"))?
        .block_on(run(args))
}

async fn run(args: Args) -> Result<(), String> {
    let manual_peers = parse_manual_peers(&args.peers)?;
    // CLI 入参统一收敛进 ConsoleConfig（库面装配契约），main 只做翻译不做业务。
    let cfg = ConsoleConfig {
        data_dir: args.data_dir.clone(),
        bootstrap: args.bootstrap.clone(),
        mdns: !args.no_mdns,
        manual_peers: manual_peers.clone(),
        agent_token: args.agent_token.clone(),
        ws_port: args.ws_port,
        status_port: args.status_port,
        reattach_window: Duration::from_secs(args.window_secs),
    };
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
    let status = StatusServer::start(
        cfg.status_port,
        local_token.clone(),
        StatusDeps {
            hub: hub.clone(),
            discovery: disc.clone(),
        },
    )
    .await
    .map_err(|e| format!("status server: {e}"))?;
    let ws = WsServer::start(
        cfg.ws_port,
        local_token.clone(),
        WsDeps {
            node: node.clone(),
            hub,
            tickets,
            window,
        },
    )
    .await
    .map_err(|e| format!("ws server: {e}"))?;

    out::event(
        "ready",
        &ReadyLine {
            ws: ws.addr.to_string(),
            status: status.addr.to_string(),
            token: local_token,
            peer: node.local_peer_id().to_string(),
        },
    );
    tracing::info!(ws = %ws.addr, status = %status.addr, "acp-console ready");

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
