//! metrics 命令域（CL4 对等补齐）：对齐 GUI metrics_get——运行时指标快照。
//! 未运行返回全零快照（同 GUI AppState::metrics 未运行分支）；运行中经
//! daemon.sock 控制通道读守护进程实时快照。GUI metrics_history 需要
//! 守护进程新增 5s 采样环行为（非薄封装），CL4 未扩权，豁免待裁决。

use clap::Args;
use clap::Subcommand;

use crate::control;
use crate::error::CliResult;
use crate::lifecycle;
use crate::node::DEFAULT_DATA_DIR;
use crate::output;
use crate::paths::Paths;
use crate::types::MetricsJson;

#[derive(Subcommand)]
pub enum MetricsCommand {
    /// 读取运行时指标快照（未运行返回全零，同 GUI）
    Get(GetArgs),
}

#[derive(Args)]
pub struct GetArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

pub async fn run(cmd: MetricsCommand) -> CliResult<()> {
    match cmd {
        MetricsCommand::Get(a) => get(a).await,
    }
}

async fn get(args: GetArgs) -> CliResult<()> {
    let snapshot = snapshot(&args.data_dir).await?;
    let text = render_text(&snapshot);
    output::emit(args.json, &snapshot, &text)
}

/// 在线走控制通道；离线返回全零（对齐 GUI 前端按零值渲染语义）。
async fn snapshot(data_dir: &str) -> CliResult<MetricsJson> {
    if lifecycle::probe_online(data_dir).await.is_none() {
        return Ok(MetricsJson::default());
    }
    let data = control::call(
        &Paths::new(data_dir),
        serde_json::json!({ "op": "metrics" }),
    )
    .await?;
    serde_json::from_value(data)
        .map_err(|e| crate::error::CliError::Runtime(format!("守护进程指标响应解析失败: {e}")))
}

fn render_text(m: &MetricsJson) -> String {
    let fields = [
        ("dialDirectOk", m.dial_direct_ok),
        ("dialDirectFail", m.dial_direct_fail),
        ("dialPunchOk", m.dial_punch_ok),
        ("dialPunchFail", m.dial_punch_fail),
        ("dialRelayOk", m.dial_relay_ok),
        ("dialRelayFail", m.dial_relay_fail),
        ("addrDialFailures", m.addr_dial_failures),
        ("relayReconnects", m.relay_reconnects),
        ("gateDenialsTotal", m.gate_denials_total),
        ("activeConnections", m.active_connections),
        ("relaySessionsActive", m.relay_sessions_active),
    ];
    fields
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_snapshot_is_all_zero() {
        let zeros = MetricsJson::default();
        assert_eq!(zeros.dial_direct_ok, 0);
        assert!(render_text(&zeros).contains("activeConnections=0"));
        assert_eq!(render_text(&zeros).lines().count(), 11);
    }

    #[test]
    fn text_keys_match_camel_case_json_keys() {
        let json = serde_json::to_value(MetricsJson::default()).unwrap();
        for line in render_text(&MetricsJson::default()).lines() {
            let key = line.split('=').next().unwrap();
            assert!(json.get(key).is_some(), "文本键 {key} 在 JSON 中缺失");
        }
    }
}
