//! acp-agent 入口：装配配置/日志/节点/handler，等信号优雅关停。

use std::path::PathBuf;
use std::sync::Arc;

use acp_agent::cli::Cli;
use acp_agent::{AcpHandler, PeerBook, SessionDeps, TracingAudit};
use clap::Parser;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli).await {
        eprintln!("acp-agent: {err}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), String> {
    let config = acp_agent::cli::assemble(&cli).map_err(|err| err.to_string())?;
    let paths = config.paths();
    ensure_dir(paths.root.as_path())?;
    init_log(&paths.root);
    let node = p2p::Node::builder()
        .quic_port(config.quic_port)
        .tcp_port(config.tcp_port)
        .mdns(true)
        .data_dir(node_identity_dir(&paths))
        .build()
        .await
        .map_err(|err| format!("node build: {err}"))?;
    let peers = PeerBook::spawn(node.events());
    let deps = SessionDeps::assemble(config.clone(), Arc::new(TracingAudit), peers)
        .map_err(|err| format!("policy load: {err}"))?;
    let handler = AcpHandler::new(deps.clone()).map_err(|err| format!("protocol id: {err}"))?;
    node.handle_protocol(Arc::new(handler));
    eprintln!(
        "acp-agent: running peer={} data-dir={}",
        node.local_peer_id(),
        paths.root.display(),
    );
    wait_shutdown().await;
    node.shutdown();
    // 桥自身退出（设计 §7）：全部子进程走退出阶梯，kill_on_drop 仅兜底
    deps.slots
        .shutdown_all(config.grace() + std::time::Duration::from_secs(2))
        .await;
    eprintln!("acp-agent: stopped");
    Ok(())
}

fn ensure_dir(dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|err| format!("create {}: {err}", dir.display()))
}

fn node_identity_dir(paths: &acp_common::AcpPaths) -> PathBuf {
    paths.root.join("identity")
}

/// 日志装配：文本格式落 <data-dir>/acp-agent.log，初始化失败回退 stderr（不阻断）。
fn init_log(root: &std::path::Path) {
    let report = p2p_log::init(p2p_log::LogConfig {
        format: p2p_log::LogFormat::Text,
        file: Some(p2p_log::FileOptions::with_default_caps(
            root,
            "acp-agent.log",
        )),
    });
    if let Some(fallback) = report.fallback {
        eprintln!("acp-agent: {fallback}");
    }
}

async fn wait_shutdown() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut term = signal(SignalKind::terminate())
        .inspect_err(|err| eprintln!("acp-agent: SIGTERM unavailable: {err}"))
        .ok();
    let term_recv = async {
        match term.as_mut() {
            Some(term) => {
                term.recv().await;
            }
            None => std::future::pending::<()>().await,
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = term_recv => {},
    }
}
