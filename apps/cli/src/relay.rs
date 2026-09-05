//! relay 命令域（F10）：中继会话/水位只读查询。数据 = metrics 快照薄封装
//! （会话/重连水位 + 降级链逐跳统计）+ 生效配置的中继端点，经控制通道读取。

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::control;
use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;
use crate::output;
use crate::paths::Paths;

#[derive(Subcommand)]
pub enum RelayCommand {
    /// 中继会话与水位快照（只读）
    Status(StatusArgs),
}

#[derive(Args)]
pub struct StatusArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

/// relay status 报告（daemon relayStatus op 同形）。
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayStatusReport {
    pub relay_sessions_active: u64,
    pub relay_reconnects: u64,
    pub active_connections: u64,
    pub dial_punch_ok: u64,
    pub dial_punch_fail: u64,
    pub dial_relay_ok: u64,
    pub dial_relay_fail: u64,
    pub relay_addrs: Vec<String>,
}

pub async fn run(cmd: RelayCommand) -> CliResult<()> {
    match cmd {
        RelayCommand::Status(a) => status(a).await,
    }
}

async fn status(args: StatusArgs) -> CliResult<()> {
    let paths = Paths::new(&args.data_dir);
    let data = control::call(&paths, json!({ "op": "relayStatus" })).await?;
    let report: RelayStatusReport = serde_json::from_value(data)
        .map_err(|e| CliError::Runtime(format!("中继状态解析失败: {e}")))?;
    let text = render(&report);
    output::emit(args.json, &report, &text)
}

/// 文本形态：首行会话/水位，逐跳统计与配置端点随后。
fn render(r: &RelayStatusReport) -> String {
    let mut lines = vec![format!(
        "relaySessionsActive={} relayReconnects={} activeConnections={}",
        r.relay_sessions_active, r.relay_reconnects, r.active_connections
    )];
    lines.push(format!(
        "dialPunch ok={} fail={}",
        r.dial_punch_ok, r.dial_punch_fail
    ));
    lines.push(format!(
        "dialRelay ok={} fail={}",
        r.dial_relay_ok, r.dial_relay_fail
    ));
    lines.extend(r.relay_addrs.iter().map(|a| format!("relayAddr={a}")));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> RelayStatusReport {
        RelayStatusReport {
            relay_sessions_active: 2,
            relay_reconnects: 1,
            active_connections: 3,
            dial_punch_ok: 4,
            dial_punch_fail: 5,
            dial_relay_ok: 6,
            dial_relay_fail: 7,
            relay_addrs: vec!["43.240.223.138/u3403".into()],
        }
    }

    #[test]
    fn text_is_key_value_greppable() {
        let text = render(&report());
        for key in [
            "relaySessionsActive=2",
            "relayReconnects=1",
            "activeConnections=3",
            "dialPunch ok=4 fail=5",
            "dialRelay ok=6 fail=7",
            "relayAddr=43.240.223.138/u3403",
        ] {
            assert!(text.contains(key), "缺 {key}: {text}");
        }
    }

    #[test]
    fn json_report_is_camel_case_parseable() {
        let value = serde_json::to_value(&report()).unwrap();
        assert_eq!(value["relaySessionsActive"], serde_json::json!(2));
        assert_eq!(value["relayAddrs"][0], serde_json::json!("43.240.223.138/u3403"));
    }
}
