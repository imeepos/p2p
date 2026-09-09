//! p2pctl authz bind/unbind：peer 级单值绑定（upsert），--expires Unix 秒。

use clap::Args;

use p2p_cli::authz::{bind, unbind, BindReport, UnbindReport};

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;
use crate::output;

use super::runtime_err;

#[derive(Args)]
pub struct BindArgs {
    /// 借权方 PeerId（base58）
    pub peer_id: String,
    /// 授予的角色 id（内建或自定义，角色必须已存在）
    pub role_id: String,
    /// 授权到期 Unix 秒（缺省 = 不过期；到期后判定 Deny(Expired)）
    #[arg(long)]
    pub expires: Option<u64>,
    /// 备注
    #[arg(long)]
    pub note: Option<String>,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Args)]
pub struct UnbindArgs {
    /// 借权方 PeerId（base58）
    pub peer_id: String,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

pub fn bind_cmd(args: BindArgs) -> CliResult<()> {
    let report: BindReport = bind(
        &args.data_dir,
        &args.peer_id,
        &args.role_id,
        args.expires,
        args.note.as_deref(),
    )
    .map_err(runtime_err)?;
    output::emit(args.json, &report, &render_bind(&report))
}

pub fn unbind_cmd(args: UnbindArgs) -> CliResult<()> {
    let report: UnbindReport = unbind(&args.data_dir, &args.peer_id).map_err(runtime_err)?;
    output::emit(
        args.json,
        &report,
        &format!(
            "已解绑 peer={}（原角色={}，回到默认拒绝）",
            report.peer_id, report.role_id
        ),
    )
}

fn render_bind(report: &BindReport) -> String {
    let state = if report.created {
        "新建绑定"
    } else {
        "条目已存在，本次为更新"
    };
    let mut lines = vec![format!(
        "已绑定 peer={} role={}（{state}）\ngranted_at={}",
        report.peer_id, report.role_id, report.granted_at
    )];
    if let Some(expires_at) = report.expires_at {
        lines.push(format!("expires_at={expires_at}"));
    }
    lines.join("\n")
}
