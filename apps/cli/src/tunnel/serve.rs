//! tunnel serve 运行时（W-T5）：headless 被访侧前台常驻进程。进程活 = 受理
//! 开启（装配即 enabled，规范页 §5.2「按次开启」的进程级形态）；SIGINT/SIGTERM
//! 优雅收口：先关新建流（shutdown 码），再等在途会话逐条落终态审计，超时显式
//! 报错非零退出（禁静默吞错）。就绪信息以 {"kind":"ready",...} JSON 行发 stdout
//! （acp console 先例），stderr 日志含会话审计八字段结构化输出。

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::Args;
use p2p::Node;
use p2p_tunnel::{TcpDialer, TunnelAudit, TunnelGate, TunnelResponder, TunnelServeConfig};

use crate::error::{CliError, CliResult};

/// 在途会话收口等待上限：本地 loopback 会话毫秒级收口，10s 为宽松护栏。
const DRAIN_TIMEOUT: Duration = Duration::from_secs(10);
const DRAIN_POLL: Duration = Duration::from_millis(100);

#[derive(Args)]
pub struct ServeArgs {
    /// 允许被隧道访问的本地目标，精确 `127.0.0.1:<port>` 字面量，可多次
    #[arg(long = "target", value_name = "TARGET", required = true)]
    pub targets: Vec<String>,
    /// 追加白名单目标（与 --target 同语义，可多次）
    #[arg(long = "allow", value_name = "TARGET")]
    pub allow: Vec<String>,
    /// 并发隧道许可上限（覆盖默认；缺省与 GUI 同源 p2p-tunnel 默认值）
    #[arg(long, value_name = "N")]
    pub max_concurrent: Option<usize>,
    /// 节点身份数据目录
    #[arg(long, default_value = crate::node::DEFAULT_DATA_DIR, value_name = "DIR")]
    pub data_dir: String,
    /// QUIC 监听端口（0 = 随机）
    #[arg(long, default_value_t = 0, value_name = "PORT")]
    pub quic_port: u16,
    /// TCP 监听端口（0 = 随机）
    #[arg(long, default_value_t = 0, value_name = "PORT")]
    pub tcp_port: u16,
    /// 关闭 mDNS 局域网发现
    #[arg(long)]
    pub no_mdns: bool,
    /// rendezvous bootstrap 地址（ip/u端口 或 ip/t端口），可多次
    #[arg(long, value_name = "ADDR")]
    pub bootstrap: Vec<String>,
}

/// 入参翻译：全部目标先校验后动作（任一非法即 Err，不部分生效）。
pub fn build_config(args: &ServeArgs) -> CliResult<TunnelServeConfig> {
    let mut allowlist = HashSet::new();
    for target in args.targets.iter().chain(args.allow.iter()) {
        if !p2p_tunnel::is_loopback_literal_target(target) {
            return Err(CliError::Runtime(format!(
                "--target/--allow 须为 127.0.0.1:<port> 字面量（端口 1-65535），当前: {target}"
            )));
        }
        allowlist.insert(target.clone());
    }
    let max_concurrent = match args.max_concurrent {
        None => TunnelServeConfig::default().max_concurrent,
        Some(0) => return Err(CliError::Runtime("--max-concurrent 须 ≥ 1".into())),
        Some(n) => n,
    };
    Ok(TunnelServeConfig {
        allowlist,
        max_concurrent,
        ..TunnelServeConfig::default()
    })
}

/// 前台运行到信号；stdout 只发 JSON 行，日志走 stderr。
pub async fn run(args: ServeArgs) -> CliResult<()> {
    let _ = p2p_log::init(p2p_log::LogConfig::default());
    let cfg = build_config(&args)?;
    let node = build_node(&args).await?;
    let gate = TunnelGate::new(cfg);
    let protocol = p2p_tunnel::protocol_id()
        .map_err(|e| CliError::Runtime(format!("tunnel 协议 ID 非法: {e}")))?;
    let responder = Arc::new(TunnelResponder::new(protocol, gate.clone(), TcpDialer));
    let audit = responder.audit();
    node.handle_protocol(responder);
    // 进程活 = enabled：白名单在 build_config 已全量校验，此处开闸。
    gate.set_enabled(true);
    emit_ready(&node, &gate);
    wait_signal().await;
    gate.set_enabled(false);
    drain_active(&gate).await.map_err(|e| {
        eprintln!("p2pctl-tunnel-serve: {e}");
        CliError::Runtime(e)
    })?;
    node.shutdown();
    emit_stopped(&audit);
    Ok(())
}

/// 节点装配（daemon build_node 同款缝，缺省不接公网 bootstrap）。
async fn build_node(args: &ServeArgs) -> CliResult<Node> {
    let mut builder = Node::builder()
        .quic_port(args.quic_port)
        .tcp_port(args.tcp_port)
        .mdns(!args.no_mdns)
        .data_dir(PathBuf::from(&args.data_dir));
    if !args.bootstrap.is_empty() {
        builder = builder.bootstrap(args.bootstrap.clone());
    }
    builder
        .build()
        .await
        .map_err(|e| CliError::Runtime(format!("节点启动失败: {e}")))
}

/// 就绪行：{"kind":"ready","peerId","listenAddrs","allow","maxConcurrent"}。
fn emit_ready(node: &Node, gate: &TunnelGate) {
    let status = gate.status();
    let line = serde_json::json!({
        "kind": "ready",
        "peerId": node.local_peer_id().to_string(),
        "listenAddrs": node.listen_addrs(),
        "allow": status.allow,
        "maxConcurrent": gate.cfg().max_concurrent,
    });
    println!("{line}");
}

/// 收口行：审计汇总（served/rejected/broken 计数），落终态凭证。
fn emit_stopped(audit: &TunnelAudit) {
    let records = audit.snapshot();
    let (mut served, mut rejected, mut broken) = (0usize, 0usize, 0usize);
    for record in &records {
        match record.outcome {
            p2p_tunnel::TunnelAuditOutcome::Served => served += 1,
            p2p_tunnel::TunnelAuditOutcome::Rejected(_) => rejected += 1,
            p2p_tunnel::TunnelAuditOutcome::Broken(_) => broken += 1,
        }
    }
    println!(
        "{}",
        serde_json::json!({
            "kind": "stopped",
            "sessions": records.len(),
            "served": served,
            "rejected": rejected,
            "broken": broken,
        })
    );
}

async fn wait_signal() {
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("装 SIGTERM");
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .expect("装 SIGINT");
    tokio::select! {
        _ = sigterm.recv() => {}
        _ = sigint.recv() => {}
    }
}

/// 等在途会话全部收口（active_sessions 归零 = 每条许可已归还 = 终态已落审计：
/// responder 在 admit_and_pump 内先 audit.record 再释放许可）。
async fn drain_active(gate: &TunnelGate) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + DRAIN_TIMEOUT;
    loop {
        let active = gate.status().active_sessions;
        if active == 0 {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(format!(
                "收口超时（{DRAIN_TIMEOUT:?}）：仍有 {active} 条在途会话未落终态"
            ));
        }
        tokio::time::sleep(DRAIN_POLL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(targets: &[&str]) -> ServeArgs {
        ServeArgs {
            targets: targets.iter().map(|s| s.to_string()).collect(),
            allow: vec![],
            max_concurrent: None,
            data_dir: "/tmp/td".into(),
            quic_port: 0,
            tcp_port: 0,
            no_mdns: true,
            bootstrap: vec![],
        }
    }

    #[test]
    fn builds_allowlist_from_target_and_allow() {
        let mut a = args(&["127.0.0.1:8014"]);
        a.allow = vec!["127.0.0.1:8015".to_string()];
        let cfg = build_config(&a).unwrap();
        assert_eq!(
            cfg.allowlist,
            HashSet::from(["127.0.0.1:8014".into(), "127.0.0.1:8015".into()])
        );
        assert_eq!(
            cfg.max_concurrent,
            TunnelServeConfig::default().max_concurrent,
            "缺省与 GUI 同源默认值"
        );
    }

    #[test]
    fn rejects_non_loopback_target_without_partial_effect() {
        for bad in ["localhost:80", "127.0.0.1:0", "0.0.0.0:80", "127.0.0.1"] {
            let err = build_config(&args(&[bad])).unwrap_err();
            assert!(err.to_string().contains("127.0.0.1:<port>"), "err={err}");
        }
        let mut mixed = args(&["127.0.0.1:8014", "localhost:80"]);
        mixed.allow = vec!["127.0.0.1:9".to_string()];
        assert!(build_config(&mixed).is_err(), "混入非法目标整体拒绝");
    }

    #[test]
    fn max_concurrent_override_and_zero_rejected() {
        let mut a = args(&["127.0.0.1:80"]);
        a.max_concurrent = Some(7);
        assert_eq!(build_config(&a).unwrap().max_concurrent, 7);
        a.max_concurrent = Some(0);
        assert!(build_config(&a).is_err());
    }

    #[tokio::test]
    async fn drain_waits_for_active_sessions_then_zero() {
        let gate = TunnelGate::new(TunnelServeConfig {
            allowlist: HashSet::from(["127.0.0.1:80".to_string()]),
            max_concurrent: 2,
            ..Default::default()
        });
        gate.set_enabled(true);
        let permit = gate.authorize("127.0.0.1:80").unwrap();
        let drain = tokio::spawn({
            let gate = gate.clone();
            async move { drain_active(&gate).await }
        });
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!drain.is_finished(), "许可未归还时 drain 必须等待");
        drop(permit);
        drain.await.unwrap().unwrap();
    }
}
