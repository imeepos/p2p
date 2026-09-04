//! llm-share allowlist 命令面：allow（upsert 授予）/ allowlist（查）/ deny（删）。
//! 语义默认拒绝：表无条目即不可用；granted_at 由逻辑层注 RFC 3339。

use clap::Args;

use p2p_cli::llm_share::allowlist::{self, AllowReport, AllowlistEntry, DenyReport};
use p2p_cli::llm_share::rfc3339_now;

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;
use crate::output;

use super::runtime_err;

#[derive(Args)]
pub struct AllowArgs {
    /// 借方 PeerId（base58，32 字节）
    pub peer_id: String,
    /// 模型白名单（可重复；缺省 = 不限模型）
    #[arg(long = "model")]
    pub model: Vec<String>,
    /// 备注（可选）
    #[arg(long)]
    pub note: Option<String>,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录（allowlist 在 <data-dir>/llm-share/allowlist.json）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Args)]
pub struct DenyArgs {
    /// 借方 PeerId（base58，32 字节）
    pub peer_id: String,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Args)]
pub struct ListArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

pub fn allow_cmd(args: AllowArgs) -> CliResult<()> {
    let report = allowlist::allow(
        &args.data_dir,
        &args.peer_id,
        &args.model,
        args.note.as_deref(),
        &rfc3339_now(),
    )
    .map_err(runtime_err)?;
    output::emit(args.json, &report, &render_allow(&report))
}

pub fn deny_cmd(args: DenyArgs) -> CliResult<()> {
    let report = allowlist::deny(&args.data_dir, &args.peer_id).map_err(runtime_err)?;
    output::emit(args.json, &report, &render_deny(&report))
}

pub fn list_cmd(args: ListArgs) -> CliResult<()> {
    let report = allowlist::list(&args.data_dir).map_err(runtime_err)?;
    output::emit(args.json, &report, &render_list(&report.peers))
}

fn render_allow(report: &AllowReport) -> String {
    let state = if report.created {
        "新建条目"
    } else {
        "条目已存在，本次为更新"
    };
    format!(
        "已加入 allowlist peer={}（{state}）\nmodels={}\nnote={}\ngranted_at={}",
        report.peer_id,
        join_models(&report.models),
        report.note,
        report.granted_at
    )
}

fn render_deny(report: &DenyReport) -> String {
    format!("已移出 allowlist peer={}（回到默认拒绝）", report.peer_id)
}

fn render_list(entries: &[AllowlistEntry]) -> String {
    if entries.is_empty() {
        return "allowlist 为空（默认拒绝：未列入条目的借方一律不可用）".to_owned();
    }
    let mut lines = vec![format!("共 {} 条 allowlist 条目", entries.len())];
    for entry in entries {
        lines.push(format!(
            "{}  models={}  note={}  granted_at={}",
            entry.peer_id,
            join_models(&entry.models),
            entry.note,
            entry.granted_at
        ));
    }
    lines.join("\n")
}

fn join_models(models: &[String]) -> String {
    if models.is_empty() {
        "不限".to_owned()
    } else {
        models.join(",")
    }
}
