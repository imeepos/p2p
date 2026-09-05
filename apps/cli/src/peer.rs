//! peer 命令域：对齐 GUI peer_dial/connect/disconnect/ping，全部经控制通道
//! 由运行中守护进程执行（节点未启动报错退出码 1）。dial/ping 走真实网络，放宽超时。
//! peer list（F10 只读观测：地址簿 + 在线态）在 peer/list.rs 子模块。

use std::time::Duration;

use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::control;
use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;
use crate::output;
use crate::paths::Paths;
use crate::types::{DialReport, PingOutcome};

mod list;

/// dial/ping 可能走降级链（直连→打洞→中继），客户端等待上限放宽到 60s。
const SLOW_OP_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Subcommand)]
pub enum PeerCommand {
    /// 拨号 "<peer_id>@<addr>"（addr 为 ip/u端口 或 ip/t端口）。
    /// 输出 hops 为降级链逐跳报告：direct=按地址簿直连尝试；punch=经中继
    /// 信令打洞；relay=中继电路兜底；hops=[] 表示复用池内已有连接，本次
    /// 未发生新拨号（不代表无路径）。
    Dial(TargetArgs),
    /// 按地址簿连接已知节点
    Connect(PeerArgs),
    /// 挂断与该节点的连接（幂等）
    Disconnect(PeerArgs),
    /// echo 协议测 RTT（亚毫秒以 rttMicros 与 0.1ms 精度呈现，F13）
    Ping(PingArgs),
    /// 列出地址簿与在线态（守护进程观测注册表只读查询）
    List(list::ListArgs),
}

/// 各子命令共用的输出/定位参数（形态同 log 域 common flatten）。
#[derive(Args)]
struct CommonArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct TargetArgs {
    /// 拨号目标 <peer_id>@<addr>
    target: String,
    #[command(flatten)]
    common: CommonArgs,
}

#[derive(Args)]
pub struct PeerArgs {
    /// 对端 PeerId（base58）
    peer_id: String,
    #[command(flatten)]
    common: CommonArgs,
}

#[derive(Args)]
pub struct PingArgs {
    /// 对端 PeerId（base58）
    peer_id: String,
    /// echo 超时毫秒
    #[arg(long, default_value_t = 5000)]
    timeout_ms: u64,
    #[command(flatten)]
    common: CommonArgs,
}

/// ping 结果视图：契约 PingOutcome 之上补 rttMicros（F13 亚毫秒精度）。
#[derive(serde::Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PingView {
    #[serde(flatten)]
    outcome: PingOutcome,
    rtt_micros: Option<u64>,
}

pub async fn run(cmd: PeerCommand) -> CliResult<()> {
    match cmd {
        PeerCommand::Dial(a) => dial(a).await,
        PeerCommand::Connect(a) => connect(a).await,
        PeerCommand::Disconnect(a) => disconnect(a).await,
        PeerCommand::Ping(a) => ping(a).await,
        PeerCommand::List(a) => list::run(a).await,
    }
}

async fn dial(args: TargetArgs) -> CliResult<()> {
    let paths = Paths::new(&args.common.data_dir);
    let data = control::call_slow(
        &paths,
        json!({ "op": "dial", "target": args.target }),
        SLOW_OP_TIMEOUT,
    )
    .await?;
    finish_dial(args.common.json, &args.target, data)
}

async fn connect(args: PeerArgs) -> CliResult<()> {
    let paths = Paths::new(&args.common.data_dir);
    let data = control::call_slow(
        &paths,
        json!({ "op": "connect", "peerId": args.peer_id }),
        SLOW_OP_TIMEOUT,
    )
    .await?;
    finish_dial(args.common.json, &args.peer_id, data)
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
    let paths = Paths::new(&args.common.data_dir);
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
    output::emit(args.common.json, &data, &text)
}

async fn ping(args: PingArgs) -> CliResult<()> {
    let paths = Paths::new(&args.common.data_dir);
    let data = control::call_slow(
        &paths,
        json!({ "op": "ping", "peerId": args.peer_id, "timeoutMs": args.timeout_ms }),
        SLOW_OP_TIMEOUT,
    )
    .await?;
    let view: PingView = serde_json::from_value(data)
        .map_err(|e| CliError::Runtime(format!("测距结果解析失败: {e}")))?;
    let outcome = &view.outcome;
    let text = match (&outcome.ok, &outcome.rtt_ms) {
        (true, Some(_)) => format!(
            "pong from {} {}",
            args.peer_id,
            render_rtt(view.rtt_micros, outcome.rtt_ms)
        ),
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
    output::emit(args.common.json, &view, &text)?;
    if !outcome.ok {
        return Err(CliError::Runtime(format!("ping {} 失败", args.peer_id)));
    }
    Ok(())
}

/// 亚毫秒 RTT 呈现（F13）：<1ms 用 0.1ms 精度并附 rtt_us，避免恒 0。
fn render_rtt(micros: Option<u64>, rtt_ms: Option<u64>) -> String {
    match micros {
        Some(us) if us < 1000 => format!("rtt_ms=0.{} rtt_us={us}", us / 100),
        Some(us) => format!("rtt_ms={} rtt_us={us}", rtt_ms.unwrap_or(0)),
        None => format!("rtt_ms={}", rtt_ms.unwrap_or(0)),
    }
}

fn render_hops(hops: &[crate::types::DialHopJson]) -> String {
    hops.iter()
        .map(|h| format!("hop={:?} ok={} detail={}", h.hop, h.ok, h.detail))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_ms_rtt_renders_tenth_precision() {
        // F13：亚毫秒不再恒 0，0.1ms 精度 + rtt_us 原值。
        assert_eq!(render_rtt(Some(412), Some(0)), "rtt_ms=0.4 rtt_us=412");
        assert_eq!(render_rtt(Some(1050), Some(1)), "rtt_ms=1 rtt_us=1050");
        assert_eq!(render_rtt(None, Some(9)), "rtt_ms=9");
    }

    #[test]
    fn ping_view_flattens_contract_fields() {
        let view: PingView = serde_json::from_value(json!({
            "ok": true,
            "rttMs": 0,
            "rttMicros": 412,
            "hops": [],
        }))
        .unwrap();
        assert!(view.outcome.ok);
        assert_eq!(view.rtt_micros, Some(412));
    }
}
