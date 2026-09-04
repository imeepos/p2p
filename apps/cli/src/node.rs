//! node 命令域。CL1 仅纵切 status：基于状态目录 + 守护进程标记判定，
//! 真实探测（心跳/端口/metrics）后续波增强；判定依据随输出可观测。

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::error::CliResult;
use crate::output;

/// facade 与实验工具约定的默认状态目录（crates/p2p NodeBuilder::default、p2p-cli --data）。
pub const DEFAULT_DATA_DIR: &str = "./p2p-data";

/// 守护进程标记文件名（CL2 启停域落地后由 start 写入 / stop 清除）。
const DAEMON_PID_FILE: &str = "daemon.pid";

#[derive(Subcommand)]
pub enum NodeCommand {
    /// 查询本机节点运行状态
    Status(StatusArgs),
}

#[derive(Args)]
pub struct StatusArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// 节点状态目录（与 facade 默认一致）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

/// 探测结论：文本/JSON 两形态共用同一事实源。
#[derive(Serialize)]
struct NodeStatus {
    running: bool,
    state: &'static str,
    reason: String,
    data_dir: String,
}

struct Probe {
    data_dir_exists: bool,
    pid_file_exists: bool,
}

pub async fn run(cmd: NodeCommand) -> CliResult {
    match cmd {
        NodeCommand::Status(args) => status(args),
    }
}

fn status(args: StatusArgs) -> CliResult {
    let probe = probe(&args.data_dir);
    let why = reason(&args.data_dir, &probe);
    let text = format!("节点未运行（{why}）");
    let status = NodeStatus {
        running: false,
        state: "not_running",
        reason: why,
        data_dir: args.data_dir,
    };
    output::emit(args.json, &status, &text)
}

fn probe(data_dir: &str) -> Probe {
    let root = std::path::Path::new(data_dir);
    Probe {
        data_dir_exists: root.is_dir(),
        pid_file_exists: root.join(DAEMON_PID_FILE).is_file(),
    }
}

fn reason(data_dir: &str, p: &Probe) -> String {
    match (p.data_dir_exists, p.pid_file_exists) {
        (_, true) => "发现守护进程标记，运行中节点探测能力后续波接入".to_string(),
        (true, false) => format!("状态目录 {data_dir} 存在，但无运行中守护进程"),
        (false, false) => format!("无状态目录 {data_dir}，无守护进程标记"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_running_reason_cites_missing_state() {
        let r = reason("./p2p-data", &Probe { data_dir_exists: false, pid_file_exists: false });
        assert!(r.contains("无状态目录"));
        assert!(r.contains("./p2p-data"));
    }

    #[test]
    fn json_form_has_running_false() {
        let s = NodeStatus {
            running: false,
            state: "not_running",
            reason: "x".into(),
            data_dir: "./p2p-data".into(),
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["running"], serde_json::json!(false));
        assert_eq!(v["state"], serde_json::json!("not_running"));
    }
}
