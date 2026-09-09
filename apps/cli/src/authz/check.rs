//! p2pctl authz check：dry-run 判定（§7），只读不写；Deny 输出 reason 码。

use clap::Args;

use p2p_cli::authz::{check, CheckReport};

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;
use crate::output;

use super::runtime_err;

#[derive(Args)]
pub struct CheckArgs {
    /// 被判定的 PeerId（base58）
    pub peer_id: String,
    /// 权限 key（§4 闭集，如 acp.session / llm.borrow）
    pub permission: String,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

pub fn check_cmd(args: CheckArgs) -> CliResult<()> {
    let report: CheckReport =
        check(&args.data_dir, &args.peer_id, &args.permission).map_err(runtime_err)?;
    output::emit(args.json, &report, &render_check(&report))
}

fn render_check(report: &CheckReport) -> String {
    let verdict = match report.reason {
        None => "Allow".to_owned(),
        Some(reason) => format!("Deny（reason={reason}）"),
    };
    format!(
        "peer={} perm={} => {}",
        report.peer_id, report.permission, verdict
    )
}
