//! 守护进程运行时（node serve 隐藏子命令）：装配节点、落 pid/meta/log 可观测信号、
//! 经 daemon.sock 服务控制请求；SIGTERM/SIGINT 优雅关停并清理现场。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use p2p::Node;
use serde_json::{json, Value};
use tokio::net::UnixListener;

use crate::error::{CliError, CliResult};
use crate::ops;
use crate::paths::{remove_file_if_exists, Paths};
use crate::store;
use crate::types::{default_bootstrap, default_observation_addrs, default_relay_addrs, GuiConfig};

/// 空列表回落出厂默认：serde 只兜字段缺失，显式 [] 在装配时兜底（GUI 同规则）。
fn with_factory_fallback(list: &[String], factory: fn() -> Vec<String>) -> Vec<String> {
    if list.is_empty() {
        factory()
    } else {
        list.to_vec()
    }
}

/// GuiConfig → Node 装配（与 GUI build_node 同构）。
async fn build_node(cfg: &GuiConfig) -> Result<Node, String> {
    let mut builder = p2p::Node::builder()
        .quic_port(cfg.quic_port)
        .tcp_port(cfg.tcp_port)
        .bootstrap(with_factory_fallback(&cfg.bootstrap, default_bootstrap))
        .mdns(cfg.enable_mdns)
        .data_dir(PathBuf::from(&cfg.data_dir))
        .relay_addrs(with_factory_fallback(&cfg.relay_addrs, default_relay_addrs))
        .advertised_addrs(cfg.advertised_addrs.clone());
    if let Some(port) = cfg.observation_port {
        builder = builder.observation_responder(port);
    }
    builder = builder.observation_addrs(with_factory_fallback(
        &cfg.observation_addrs,
        default_observation_addrs,
    ));
    builder.build().await.map_err(|e| format!("节点启动失败: {e}"))
}

/// 守护进程上下文：控制请求共享的不可变状态。
struct Ctx {
    node: Arc<Node>,
    config: GuiConfig,
    started: Instant,
    started_at_ms: u64,
    log_path: PathBuf,
}

pub async fn run(data_dir: &str) -> CliResult<()> {
    let paths = Paths::new(data_dir);
    paths
        .ensure_dir()
        .map_err(|e| CliError::Runtime(format!("创建数据目录失败: {e}")))?;
    init_log(&paths);
    let config = store::load_config(&paths);
    let node = build_node(&config)
        .await
        .map_err(CliError::Runtime)
        .inspect_err(|e| eprintln!("p2pctl-daemon: {e}"))?;
    node.handle_protocol(Arc::new(
        p2p_cli::echo::EchoHandler::new()
            .map_err(|e| CliError::Runtime(format!("echo 协议装配失败: {e}")))?,
    ));
    let ctx = Arc::new(Ctx {
        node: Arc::new(node),
        started_at_ms: now_ms(),
        started: Instant::now(),
        config,
        log_path: paths.log(),
    });
    write_runtime_files(&paths, &ctx)?;
    serve(paths, ctx).await
}

/// 日志装配：固定落 <data-dir>/daemon.log（失败回 stderr，不阻断启动）。
fn init_log(paths: &Paths) {
    let report = p2p_log::init(p2p_log::LogConfig {
        format: p2p_log::LogFormat::Text,
        file: Some(p2p_log::FileOptions::with_default_caps(
            paths.root.clone(),
            crate::paths::LOG_FILE,
        )),
    });
    if let Some(fallback) = report.fallback {
        eprintln!("p2pctl-daemon: {fallback}");
    }
}

fn write_runtime_files(paths: &Paths, ctx: &Ctx) -> CliResult<()> {
    std::fs::write(paths.pid(), std::process::id().to_string())
        .map_err(|e| CliError::Runtime(format!("写 pid 文件失败: {e}")))?;
    let meta = json!({
        "pid": std::process::id(),
        "peerId": ctx.node.local_peer_id().to_string(),
        "listenAddrs": ctx.node.listen_addrs(),
        "startedAtMs": ctx.started_at_ms,
        "dataDir": paths.root.to_string_lossy(),
        "logPath": ctx.log_path.to_string_lossy(),
    });
    std::fs::write(paths.meta(), serde_json::to_string_pretty(&meta).unwrap())
        .map_err(|e| CliError::Runtime(format!("写 meta 文件失败: {e}")))?;
    Ok(())
}

/// 服务循环：accept 控制连接，逐连接一请求一响应；信号触发优雅关停。
async fn serve(paths: Paths, ctx: Arc<Ctx>) -> CliResult<()> {
    if paths.sock().exists() {
        remove_file_if_exists(&paths.sock())
            .map_err(|e| CliError::Runtime(format!("清理残留 socket 失败: {e}")))?;
    }
    let listener = UnixListener::bind(paths.sock())
        .map_err(|e| CliError::Runtime(format!("绑定控制 socket 失败: {e}")))?;
    eprintln!(
        "p2pctl-daemon: running pid={} peer={} log={}",
        std::process::id(),
        ctx.node.local_peer_id(),
        ctx.log_path.display()
    );
    let mut sigterm =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("装 SIGTERM");
    let mut sigint =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).expect("装 SIGINT");
    let result = loop {
        tokio::select! {
            _ = sigterm.recv() => break Ok(()),
            _ = sigint.recv() => break Ok(()),
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    let ctx = ctx.clone();
                    tokio::spawn(handle_conn(stream, ctx));
                }
                Err(e) => {
                    eprintln!("p2pctl-daemon: accept 失败: {e}");
                    break Err(CliError::Runtime(format!("accept 控制连接失败: {e}")));
                }
            },
        }
    };
    shutdown(&paths, &ctx);
    result
}

fn shutdown(paths: &Paths, ctx: &Ctx) {
    ctx.node.shutdown();
    for path in [paths.sock(), paths.pid(), paths.meta()] {
        if let Err(e) = remove_file_if_exists(&path) {
            eprintln!("p2pctl-daemon: 清理 {} 失败: {e}", path.display());
        }
    }
    eprintln!("p2pctl-daemon: stopped");
}

async fn handle_conn(stream: tokio::net::UnixStream, ctx: Arc<Ctx>) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let (reader, mut writer) = stream.into_split();
    let mut line = String::new();
    if BufReader::new(reader).read_line(&mut line).await.is_err() {
        return;
    }
    let request: Value = match serde_json::from_str(line.trim()) {
        Ok(v) => v,
        Err(e) => {
            let _ = writer
                .write_all(format!("{{\"ok\":false,\"error\":\"请求解析失败: {e}\"}}\n").as_bytes())
                .await;
            return;
        }
    };
    let response = dispatch(&ctx, &request).await;
    let _ = writer.write_all(format!("{response}\n").as_bytes()).await;
}

/// 控制请求分派；Err(String) 统一包成 ok=false 响应（错误语义不静默）。
async fn dispatch(ctx: &Ctx, request: &Value) -> Value {
    let op = request.get("op").and_then(Value::as_str).unwrap_or("");
    let result = match op {
        "status" => Ok(ops::status_json(&ctx.node, &ctx.config, ctx.started, ctx.started_at_ms)),
        "dial" => {
            let target = request.get("target").and_then(Value::as_str).unwrap_or("");
            ops::dial(&ctx.node, target).await
        }
        "connect" => {
            let peer = str_arg(request, "peerId");
            ops::connect(&ctx.node, &peer).await
        }
        "disconnect" => ops::disconnect(&ctx.node, &str_arg(request, "peerId")),
        "ping" => {
            let peer = str_arg(request, "peerId");
            let timeout = request.get("timeoutMs").and_then(Value::as_u64).unwrap_or(5000);
            ops::ping(&ctx.node, &peer, timeout).await
        }
        other => Err(format!("未知操作 {other:?}")),
    };
    match result {
        Ok(data) => json!({ "ok": true, "data": data }),
        Err(e) => json!({ "ok": false, "error": e }),
    }
}

fn str_arg<'a>(request: &'a Value, key: &str) -> String {
    request
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
