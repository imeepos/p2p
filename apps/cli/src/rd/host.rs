//! rd host 前台常驻服务（M6）：headless 远程桌面被控端。
//!
//! 画面采集用合成源（SyntheticFactory）——真实 ScreenCaptureKit 采集与 CGEvent
//! 注入由 GUI host 承担（需系统授权），本命令用于链路验收/开发/压测。进程活 =
//! 受理开启；SIGINT/SIGTERM 优雅收口（node.shutdown）。就绪行
//! `{"kind":"ready",...}` 发 stdout（acp console/tunnel serve 先例），日志走 stderr。

use std::path::PathBuf;
use std::sync::Arc;

use clap::Args;
use p2p::Node;
use rd_host::{HostConfig, RdHost, SyntheticFactory};

use crate::error::{CliError, CliResult};

#[derive(Args)]
pub struct HostArgs {
    /// 合成画面宽
    #[arg(long, default_value_t = 1280, value_name = "N")]
    pub width: u16,
    /// 合成画面高
    #[arg(long, default_value_t = 720, value_name = "N")]
    pub height: u16,
    /// 视频帧率（1..=60；viewer 质量协商可覆盖）
    #[arg(long, default_value_t = 15, value_name = "N")]
    pub fps: u8,
    /// 编码：0=raw，1=zlib
    #[arg(long, default_value_t = 0, value_name = "N")]
    pub codec: u8,
    /// 文件隔离根（viewer 可见目录树）
    #[arg(long, value_name = "DIR")]
    pub fs_root: Option<String>,
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

/// 前台运行到信号；stdout 只发 JSON 行，日志走 stderr。
pub async fn run(args: HostArgs) -> CliResult<()> {
    let _ = p2p_log::init(p2p_log::LogConfig::default());
    if !(1..=60).contains(&args.fps) {
        return Err(CliError::Runtime("--fps 须在 1..=60".into()));
    }
    if args.codec != 0 && args.codec != 1 {
        return Err(CliError::Runtime("--codec 须为 0(raw) 或 1(zlib)".into()));
    }
    let fs_root = PathBuf::from(args.fs_root.clone().unwrap_or_else(default_fs_root));
    let node = build_node(&args).await?;
    let config = HostConfig {
        fps: args.fps,
        codec: args.codec,
        fs_root,
        require_approval: false,
        ..Default::default()
    };
    let _host = RdHost::with_config(
        node.clone(),
        Arc::new(SyntheticFactory { w: args.width, h: args.height }),
        Arc::new(rd_input::recording::RecordingInjectorFactory::new()),
        Arc::new(rd_clipboard::memory::MemoryClipboardFactory::new()),
        config,
    )
    .map_err(|e| CliError::Runtime(format!("rd host 装配失败: {e}")))?;
    emit_ready(&node);
    wait_signal().await;
    node.shutdown();
    println!("{}", serde_json::json!({ "kind": "stopped" }));
    Ok(())
}

/// 节点装配（tunnel serve 同款缝）。
async fn build_node(args: &HostArgs) -> CliResult<Arc<Node>> {
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
        .map(Arc::new)
        .map_err(|e| CliError::Runtime(format!("节点启动失败: {e}")))
}

/// 就绪行。
fn emit_ready(node: &Arc<Node>) {
    println!(
        "{}",
        serde_json::json!({
            "kind": "ready",
            "peerId": node.local_peer_id().to_string(),
            "listenAddrs": node.listen_addrs(),
        })
    );
}

fn default_fs_root() -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home {
        Some(h) => h.join("Downloads").join("RD").to_string_lossy().into_owned(),
        None => ".rd-files".into(),
    }
}

#[cfg(unix)]
async fn wait_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
    let mut int = signal(SignalKind::interrupt()).expect("SIGINT handler");
    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
    }
}

#[cfg(not(unix))]
async fn wait_signal() {
    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    let _ = tokio::signal::ctrl_c().await;
}