//! peer 命令域：对齐 GUI peer_dial/connect/disconnect/ping，全部经控制通道
//! 由运行中守护进程执行（节点未启动报错退出码 1）。dial/ping 走真实网络，放宽超时。

use std::time::Duration;

use clap::{Args, Subcommand};
use serde_json::{json, Value};

use crate::control;
use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;
use crate::output;
use crate::paths::Paths;
use crate::types::{DialReport, PingOutcome};

/// dial/ping 可能走降级链（直连→打洞→中继），客户端等待上限放宽到 60s。
const SLOW_OP_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Subcommand)]
pub enum PeerCommand {
    /// 拨号 "<peer_id>@<addr>"（addr 为 ip/u端口 或 ip/t端口）
    Dial(TargetArgs),
    /// 按地址簿连接已知节点
    Connect(PeerArgs),
    /// 挂断与该节点的连接（幂等）
    Disconnect(PeerArgs),
    /// echo 协议测 RTT
    Ping(PingArgs),
}

#[derive(Args)]
pub struct TargetArgs {
    /// 拨号目标 <peer_id>@<addr>
    target: String,
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct PeerArgs {
    /// 对端 PeerId（base58）
    peer_id: String,
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct PingArgs {
    /// 对端 PeerId（base58）
    peer_id: String,
    /// echo 超时毫秒
    #[arg(long, default_value_t = 5000)]
    timeout_ms: u64,
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

pub async fn run(cmd: PeerCommand) -> CliResult<()> {
    match cmd {
        PeerCommand::Dial(a) => dial(a).await,
        PeerCommand::Connect(a) => connect(a).await,
        PeerCommand::Disconnect(a) => disconnect(a).await,
        PeerCommand::Ping(a) => ping(a).await,
    }
}

async fn dial(args: TargetArgs) -> CliResult<()> {
    let paths = Paths::new(&args.data_dir);
    let data = control::call_slow(
        &paths,
        json!({ "op": "dial", "target": args.target }),
        SLOW_OP_TIMEOUT,
    )
    .await?;
    finish_dial(args.json, &args.target, data)
}

async fn connect(args: PeerArgs) -> CliResult<()> {
    let paths = Paths::new(&args.data_dir);
    let data = control::call_slow(
        &paths,
        json!({ "op": "connect", "peerId": args.peer_id }),
        SLOW_OP_TIMEOUT,
    )
    .await?;
    finish_dial(args.json, &args.peer_id, data)
}

/// 拨号报告：ok=false 时文本照印、退出码 1（可观测失败）。
fn finish_dial(json: bool, target: &str, data: Value) -> CliResult<()> {
    let report: DialReport = serde_json::from_value(data)
        .map_err(|e| CliError::Runtime(format!("拨号报告解析失败: {e}")))?;
    let hops = render_hops(&report.hops);
    let status_line = if report.ok {
        format!(
            "已连接 {target}（totalMs={}，共 {} 跳）",
            report.total_ms,
            report.hops.len()
        )
    } else {
        format!("连接 {target} 失败（totalMs={}）", report.total_ms)
    };
    let text = if hops.is_empty() {
        status_line
    } else {
        format!("{status_line}\n{hops}")
    };
    output::emit(json, &report, &text)?;
    if !report.ok {
        return Err(CliError::Runtime(format!("连接 {target} 失败")));
    }
    Ok(())
}

async fn disconnect(args: PeerArgs) -> CliResult<()> {
    let paths = Paths::new(&args.data_dir);
    let data = control::call(
        &paths,
        json!({ "op": "disconnect", "peerId": args.peer_id }),
    )
    .await?;
    let disconnected = data
        .get("disconnected")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let peer = data
        .get("peer")
        .and_then(Value::as_str)
        .unwrap_or(&args.peer_id);
    let text = if disconnected {
        format!("已挂断 {peer}")
    } else {
        format!("{peer} 无活动连接（幂等）")
    };
    output::emit(args.json, &data, &text)
}

async fn ping(args: PingArgs) -> CliResult<()> {
    let paths = Paths::new(&args.data_dir);
    let data = control::call_slow(
        &paths,
        json!({ "op": "ping", "peerId": args.peer_id, "timeoutMs": args.timeout_ms }),
        SLOW_OP_TIMEOUT,
    )
    .await?;
    let outcome: PingOutcome = serde_json::from_value(data)
        .map_err(|e| CliError::Runtime(format!("测距结果解析失败: {e}")))?;
    let text = match (&outcome.ok, &outcome.rtt_ms) {
        (true, Some(rtt)) => format!("pong from {} rtt_ms={}", args.peer_id, rtt),
        _ => format!(
            "ping {} 失败：{}",
            args.peer_id,
            outcome.error.clone().unwrap_or_default()
        ),
    };
    let text = match render_hops(&outcome.hops) {
        hops if !hops.is_empty() => format!("{text}\n{hops}"),
        _ => text,
    };
    output::emit(args.json, &outcome, &text)?;
    if !outcome.ok {
        return Err(CliError::Runtime(format!("ping {} 失败", args.peer_id)));
    }
    Ok(())
}

fn render_hops(hops: &[crate::types::DialHopJson]) -> String {
    hops.iter()
        .map(|h| format!("hop={:?} ok={} detail={}", h.hop, h.ok, h.detail))
        .collect::<Vec<_>>()
        .join("\n")
}
