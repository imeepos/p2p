//! llm-share share 命令面（契约 §16.6 v13）：create / list / revoke。
//! 逻辑在 p2p-cli::llm_share::share；peer 取本机身份（self_peer_id）；
//! token 原文只出现在 create 输出的 link 里一次，台账只存 sha256。

use clap::{Args, Subcommand};

use p2p_cli::llm_share::now_secs;
use p2p_cli::llm_share::share::{
    self, ShareCreateParams, ShareCreateReport, ShareListEntry, ShareRevokeReport,
};

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;
use crate::output;

use super::{runtime_err, self_peer_id};

#[derive(Subcommand)]
pub enum ShareCommand {
    /// 生成一次性分享链接（token 原文只在本响应出现一次）
    Create(CreateArgs),
    /// 列出分享台账（脱敏：无 token 原文与哈希）
    List(ListArgs),
    /// 撤销分享（按 source=share:<id> 级联删 allowlist 条目）
    Revoke(RevokeArgs),
}

#[derive(Args)]
pub struct CreateArgs {
    /// provider id（分享的模型范围来源）
    #[arg(long, required = true)]
    pub provider: String,
    /// 分享模型（可重复；缺省 = provider 全部模型）
    #[arg(long = "model")]
    pub model: Vec<String>,
    /// 到期（Unix 秒；缺省 now+24h，上限 now+7d）
    #[arg(long = "expires-at")]
    pub expires_at: Option<u64>,
    /// 备注（可选）
    #[arg(long)]
    pub note: Option<String>,
    /// 链接候选地址（可重复；QUIC/TCP/中继）
    #[arg(long = "addr")]
    pub addr: Vec<String>,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录（台账在 <data-dir>/llm-share/shares.json）
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

#[derive(Args)]
pub struct RevokeArgs {
    /// 分享 id（share create 输出中的 shareId）
    pub share_id: String,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

pub fn run(command: ShareCommand) -> CliResult<()> {
    match command {
        ShareCommand::Create(args) => create_cmd(args),
        ShareCommand::List(args) => list_cmd(args),
        ShareCommand::Revoke(args) => revoke_cmd(args),
    }
}

fn create_cmd(args: CreateArgs) -> CliResult<()> {
    let peer = self_peer_id(&args.data_dir)?;
    let params = ShareCreateParams {
        peer,
        provider_id: args.provider,
        models: if args.model.is_empty() {
            None
        } else {
            Some(args.model)
        },
        expires_at_unix: args.expires_at,
        note: args.note.unwrap_or_default(),
        addrs: args.addr,
    };
    let report = share::share_create(&args.data_dir, params, now_secs()).map_err(runtime_err)?;
    output::emit(args.json, &report, &render_create(&report))
}

fn list_cmd(args: ListArgs) -> CliResult<()> {
    let report = share::share_list(&args.data_dir, now_secs()).map_err(runtime_err)?;
    output::emit(args.json, &report, &render_list(&report.shares))
}

fn revoke_cmd(args: RevokeArgs) -> CliResult<()> {
    let report = share::share_revoke(&args.data_dir, &args.share_id).map_err(runtime_err)?;
    output::emit(args.json, &report, &render_revoke(&report))
}

fn render_create(report: &ShareCreateReport) -> String {
    format!(
        "已创建分享链接 share_id={}\nlink={}\nexpires_at={}\nmodels={}",
        report.share_id,
        report.link,
        report.expires_at,
        report.models.join(",")
    )
}

fn render_list(entries: &[ShareListEntry]) -> String {
    if entries.is_empty() {
        return "分享台账为空（用 llm-share share create 创建）".to_owned();
    }
    let mut lines = vec![format!("共 {} 条分享", entries.len())];
    for entry in entries {
        lines.push(format!(
            "share_id={}  status={}  provider_id={}  models={}  activations={}  expires_at={}  revoked={}  bound_peer={}  note={}  created_at={}",
            entry.share_id,
            entry.status,
            entry.provider_id,
            entry.models.join(","),
            entry.activations,
            entry.expires_at,
            entry.revoked,
            entry.bound_peer.as_deref().unwrap_or("-"),
            entry.note,
            entry.created_at
        ));
    }
    lines.join("\n")
}

fn render_revoke(report: &ShareRevokeReport) -> String {
    format!(
        "已撤销分享 share_id={}（allowlist 级联移除 {} 条 share 来源条目）",
        report.share_id, report.allowlist_removed
    )
}
