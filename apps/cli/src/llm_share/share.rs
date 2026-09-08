//! llm-share share 命令面（契约 §16.6 v13）：create / list / revoke / redeem。
//! create/list/revoke 逻辑在 p2p-cli::llm_share::share，redeem 编排在其
//! share_redeem 模块；create 的 peer 取本机身份（self_peer_id）；token 原文
//! 只出现在 create 输出的 link 里一次，台账只存 sha256。

use clap::{Args, Subcommand};

use p2p_cli::llm_share::now_secs;
use p2p_cli::llm_share::share::{
    self, ShareCreateParams, ShareCreateReport, ShareListEntry, ShareRevokeReport,
};
use p2p_cli::llm_share::share_redeem::{self, RedeemOutcome, RedeemParams, RedeemStatus};

use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;
use crate::output;
use crate::paths::Paths;
use crate::store;

use super::{runtime_err, self_peer_id};

#[derive(Subcommand)]
pub enum ShareCommand {
    /// 生成一次性分享链接（token 原文只在本响应出现一次）
    Create(CreateArgs),
    /// 列出分享台账（脱敏：无 token 原文与哈希）
    List(ListArgs),
    /// 撤销分享（按 source=share:<id> 级联删 allowlist 条目）
    Revoke(RevokeArgs),
    /// 兑换分享链接（借方：一次性拨号，认证 PeerId 进出借方 allowlist）
    Redeem(RedeemArgs),
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

#[derive(Args)]
pub struct RedeemArgs {
    /// dsh-llm-share:// 链接原文（聊天消息中的链接）
    pub link: String,
    /// 兑换单步（拨号/请求/应答）超时秒
    #[arg(long, default_value_t = share_redeem::REDEEM_TIMEOUT_SECS)]
    pub timeout_secs: u64,
    /// rendezvous 查号等待秒（链接无 addr 时查号用）
    #[arg(long, default_value_t = share_redeem::REDEEM_DISCOVER_SECS)]
    pub discover_secs: u64,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录（借方身份种子与节点目录同根派生）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

pub async fn run(command: ShareCommand) -> CliResult<()> {
    match command {
        ShareCommand::Create(args) => create_cmd(args),
        ShareCommand::List(args) => list_cmd(args),
        ShareCommand::Revoke(args) => revoke_cmd(args),
        ShareCommand::Redeem(args) => redeem_cmd(args).await,
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

/// 兑换（借方）：bootstrap 取节点配置（rendezvous 查号同源），节点目录与身份
/// 种子同根派生（兑换绑定本机 PeerId）。拒绝是业务结果：报告照常输出后退出 1。
async fn redeem_cmd(args: RedeemArgs) -> CliResult<()> {
    let paths = Paths::new(&args.data_dir);
    let cfg = store::load_config(&paths);
    let params = RedeemParams {
        link: args.link,
        bootstrap: cfg.bootstrap.clone(),
        node_dir: paths
            .node_data_dir(Some(&cfg.data_dir))
            .display()
            .to_string(),
        timeout_secs: args.timeout_secs,
        discover_secs: args.discover_secs,
    };
    let outcome = share_redeem::run(&params).await.map_err(runtime_err)?;
    output::emit(args.json, &outcome, &render_redeem(&outcome))?;
    if outcome.status == RedeemStatus::Rejected {
        return Err(CliError::Runtime(format!(
            "兑换被拒（{}）",
            outcome.code.as_deref().unwrap_or("unknown")
        )));
    }
    Ok(())
}

fn render_redeem(outcome: &RedeemOutcome) -> String {
    let mut lines = vec![format!("status={}", status_str(outcome))];
    if let Some(code) = &outcome.code {
        lines.push(format!("code={code}"));
    }
    if let Some(share_id) = &outcome.share_id {
        lines.push(format!("share_id={share_id}"));
    }
    if let Some(owner) = &outcome.owner {
        lines.push(format!("owner={owner}"));
    }
    if let Some(offer) = &outcome.offer {
        lines.push(format!("peer={}", offer.peer));
        lines.push(format!("models={}", offer.models.join(",")));
        let spare = offer
            .spare
            .iter()
            .map(|(model, n)| format!("{model}={n}"))
            .collect::<Vec<_>>()
            .join(",");
        lines.push(format!("spare={spare}"));
        lines.push(format!("period_ends={}", offer.period_ends));
    }
    lines.join("\n")
}

fn status_str(outcome: &RedeemOutcome) -> &'static str {
    match outcome.status {
        RedeemStatus::Redeemed => "redeemed",
        RedeemStatus::Rejected => "rejected",
    }
}
