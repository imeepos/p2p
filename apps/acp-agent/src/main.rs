//! acp-agent 入口：装配配置/日志/节点/handler，等信号优雅关停。

use std::path::PathBuf;
use std::sync::Arc;

use acp_agent::cli::Cli;
use acp_agent::{AcpHandler, SessionDeps, TracingAudit};
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
    let deps = SessionDeps::assemble(config.clone(), Arc::new(TracingAudit))
        .map_err(|err| format!("policy load: {err}"))?;
    let handler = AcpHandler::new(deps.clone()).map_err(|err| format!("protocol id: {err}"))?;
    node.handle_protocol(Arc::new(handler));
    start_admin(&config, &paths, &node, deps.clone()).await?;
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

/// 本地 admin HTTP 装配（设计 §5）：token 随机生成落 0600 文件，
/// 端口与 token 文件经 stdout JSON 行发布；--admin-disabled 可关。
async fn start_admin(
    config: &acp_agent::AgentConfig,
    paths: &acp_common::AcpPaths,
    node: &p2p::Node,
    deps: Arc<acp_agent::SessionDeps>,
) -> Result<(), String> {
    if config.admin_disabled {
        eprintln!("acp-agent: admin http disabled (--admin-disabled)");
        return Ok(());
    }
    let token = acp_agent::share::admin::AdminToken::issue(paths.admin_token())
        .map_err(|err| format!("admin token file: {err}"))?;
    let server = acp_agent::share::admin::AdminServer::start(
        config.admin_port,
        token.value.clone(),
        acp_agent::share::admin::AdminDeps {
            service: deps.shares.clone(),
            link: acp_agent::LinkContext {
                peer: node.local_peer_id().to_string(),
                addrs: node.listen_addrs(),
            },
            workspaces: config.workspace_rows(),
        },
    )
    .await
    .map_err(|err| format!("admin http: {err}"))?;
    println!(
        "{}",
        acp_agent::share::admin::ready_line(server.addr.port(), &token.file)
    );
    publish_local_descriptor(config, node, server.addr.port(), &token.value);
    Ok(())
}

/// 本机自描述落盘（2026-09-07 用户裁决：GUI 免手填 admin token）：写到用户级
/// 约定路径 ~/.dsh/acp/local-agent.json（0600），GUI 自动登记本机管理端点。
/// 失败仅告警不阻断 admin 通道（显式可观测，不静默）。
fn publish_local_descriptor(
    config: &acp_agent::AgentConfig,
    node: &p2p::Node,
    admin_port: u16,
    token_value: &str,
) {
    let Some(home) = acp_common::user_home_dir() else {
        tracing::warn!("用户主目录不可得，跳过本机描述文件：GUI 将无法自动发现 admin 端点");
        return;
    };
    let descriptor = build_local_descriptor(
        &config.agent_name,
        &node.local_peer_id().to_string(),
        admin_port,
        token_value,
    );
    if let Err(err) = acp_common::write_descriptor(&home, &descriptor) {
        tracing::warn!(error = %err, "本机描述文件写入失败");
    }
}

fn build_local_descriptor(
    agent_name: &str,
    peer: &str,
    admin_port: u16,
    token_value: &str,
) -> acp_common::LocalAgentDescriptor {
    acp_common::LocalAgentDescriptor {
        version: acp_common::DESCRIPTOR_VERSION,
        admin_url: format!("http://127.0.0.1:{admin_port}"),
        token: token_value.to_owned(),
        peer: peer.to_owned(),
        agent_name: agent_name.to_owned(),
        written_at_unix: acp_common::unix_now(),
    }
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

#[cfg(test)]
mod descriptor_tests {
    use super::build_local_descriptor;

    #[test]
    fn descriptor_carries_loopback_admin_url_and_version() {
        let descriptor = build_local_descriptor("home-agent", "12D3KooW", 8123, "tok");
        assert_eq!(descriptor.admin_url, "http://127.0.0.1:8123");
        assert_eq!(descriptor.version, acp_common::DESCRIPTOR_VERSION);
        assert_eq!(descriptor.peer, "12D3KooW");
        assert_eq!(descriptor.token, "tok");
    }
}
