//! llm-share offer 命令面：publish（组装 + 节点身份签名 + 原子落盘）/
//! show（当前生效声明 + 剩余 TTL 与生效状态）。

use clap::{Args, Subcommand};

use p2p_cli::llm_share::now_secs;
use p2p_cli::llm_share::offer::{self, OfferParams, OfferReport, OfferShowReport};

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;
use crate::output;

use super::{runtime_err, seed_path};

#[derive(Subcommand)]
pub enum OfferCommand {
    /// 组装能力声明并以本机身份签名发布（写，须人确认）
    Publish(PublishArgs),
    /// 查看当前声明与剩余 TTL
    Show(ShowArgs),
}

#[derive(Args)]
pub struct PublishArgs {
    /// 出借模型（可重复）
    #[arg(long = "model", required = true)]
    pub model: Vec<String>,
    /// 模型闲量声明 token 数（model=N，可重复，须覆盖全部 --model）
    #[arg(long = "spare", required = true)]
    pub spare: Vec<String>,
    /// 账期截止日（YYYY-MM-DD）
    #[arg(long = "period-ends")]
    pub period_ends: String,
    /// 单请求 max_tokens 上限（model=N，可重复；缺省 = 未显式设限）
    #[arg(long = "max-per-req")]
    pub max_per_req: Vec<String>,
    /// 每分钟请求上限
    #[arg(long, default_value = "10")]
    pub rpm: u32,
    /// 并发上限
    #[arg(long, default_value = "2")]
    pub concurrency: u32,
    /// 声明有效期（秒），自签发起算
    #[arg(long, default_value = "3600")]
    pub ttl: u64,
    /// 数据留存自述（§7.3 如实告知，缺省 none）
    #[arg(long, default_value = "none")]
    pub retention: String,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录（声明信封在 <data-dir>/llm-share/offer.json）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Args)]
pub struct ShowArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

pub fn run(command: OfferCommand) -> CliResult<()> {
    match command {
        OfferCommand::Publish(args) => publish_cmd(args),
        OfferCommand::Show(args) => show_cmd(args),
    }
}

fn publish_cmd(args: PublishArgs) -> CliResult<()> {
    let seed = seed_path(&args.data_dir)?;
    let params = OfferParams {
        models: args.model,
        spare: args.spare,
        period_ends: args.period_ends,
        max_per_req: args.max_per_req,
        rpm: args.rpm,
        concurrency: args.concurrency,
        ttl_secs: args.ttl,
        retention: Some(args.retention),
    };
    let report = offer::publish(&seed, &args.data_dir, &params, now_secs()).map_err(runtime_err)?;
    output::emit(args.json, &report, &render(&report, None))
}

fn show_cmd(args: ShowArgs) -> CliResult<()> {
    let report = offer::show(&args.data_dir, now_secs()).map_err(runtime_err)?;
    output::emit(args.json, &report, &render(&report.offer, Some(&report)))
}

fn render(report: &OfferReport, view: Option<&OfferShowReport>) -> String {
    let mut lines = vec![
        format!("peer={}", report.peer),
        format!("models={}", report.models.join(",")),
        format!(
            "spare={}",
            report
                .spare
                .iter()
                .map(|(m, v)| format!("{m}={v}"))
                .collect::<Vec<_>>()
                .join(",")
        ),
        format!("period_ends={}", report.period_ends),
        format!(
            "max_per_req={}",
            report
                .max_per_req
                .iter()
                .map(|(m, v)| format!("{m}={v}"))
                .collect::<Vec<_>>()
                .join(",")
        ),
        format!(
            "rate_limit=rpm={},concurrency={}",
            report.rate_limit.rpm, report.rate_limit.concurrency
        ),
        format!("ttl={}s", report.ttl),
        format!("retention={}", report.retention),
        format!("issued_at={}", report.issued_at),
        format!("expires_at={}", report.expires_at),
    ];
    if let Some(view) = view {
        lines.push(format!("status={}", view.status));
        lines.push(format!("remaining_secs={}", view.remaining_secs));
    }
    lines.push(format!("file={}", report.file));
    lines.join("\n")
}
