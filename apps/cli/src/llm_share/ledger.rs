//! llm-share ledger 命令面：list（双边流水明细）/ balance（净差视图，
//! 按 lender+period 切分，符号规则对齐账本 net：本机为 lender 记正、borrower 记负）。

use clap::{Args, Subcommand};

use p2p_cli::llm_share::ledger::{self, BalanceReport, LedgerFilters, LedgerListReport};

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;
use crate::output;

use super::{runtime_err, self_peer_id};

#[derive(Subcommand)]
pub enum LedgerCommand {
    /// 查询本机流水明细（可按 lender/borrower/period 过滤）
    List(ListArgs),
    /// 净差视图（本机参与的双边条目，按 lender+period 聚合）
    Balance(BalanceArgs),
}

#[derive(Args)]
pub struct ListArgs {
    /// 只看出借方为该 PeerId 的流水
    #[arg(long)]
    pub lender: Option<String>,
    /// 只看借方为该 PeerId 的流水
    #[arg(long)]
    pub borrower: Option<String>,
    /// 只看该账期（如 2026-09）
    #[arg(long)]
    pub period: Option<String>,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录（流水在 <data-dir>/llm-share/ledger.json）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Args)]
pub struct BalanceArgs {
    /// 只统计该账期（缺省 = 全部账期分行输出）
    #[arg(long)]
    pub period: Option<String>,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

pub fn run(command: LedgerCommand) -> CliResult<()> {
    match command {
        LedgerCommand::List(args) => list_cmd(args),
        LedgerCommand::Balance(args) => balance_cmd(args),
    }
}

fn list_cmd(args: ListArgs) -> CliResult<()> {
    let filters = LedgerFilters {
        lender: args.lender.as_deref(),
        borrower: args.borrower.as_deref(),
        period: args.period.as_deref(),
    };
    let report = ledger::list(&args.data_dir, filters).map_err(runtime_err)?;
    output::emit(args.json, &report, &render_list(&report))
}

fn balance_cmd(args: BalanceArgs) -> CliResult<()> {
    let peer = self_peer_id(&args.data_dir)?;
    let report =
        ledger::balance(&args.data_dir, &peer, args.period.as_deref()).map_err(runtime_err)?;
    output::emit(args.json, &report, &render_balance(&report))
}

fn render_list(report: &LedgerListReport) -> String {
    if report.entries.is_empty() {
        return "流水为空（<data-dir>/llm-share/ledger.json 无收据记录）".to_owned();
    }
    let mut lines = vec![format!("共 {} 条流水", report.count)];
    for entry in &report.entries {
        lines.push(format!(
            "req_id={}  period={}  lender={}  borrower={}  model={}  tokens={} (in={} out={})  estimated={}  ts={}",
            entry.req_id,
            entry.period,
            entry.lender,
            entry.borrower,
            entry.model,
            entry.tokens,
            entry.input,
            entry.output,
            entry.estimated,
            entry.ts
        ));
    }
    lines.join("\n")
}

fn render_balance(report: &BalanceReport) -> String {
    if report.rows.is_empty() {
        return format!("peer={} 无本机参与的双边流水", report.peer);
    }
    let mut lines = vec![format!(
        "peer={} 净差视图（lender 正 / borrower 负）",
        report.peer
    )];
    for row in &report.rows {
        lines.push(format!(
            "lender={}  period={}  lent_out={}  borrowed={}  net={}  entries={}",
            row.lender, row.period, row.lent_out, row.borrowed, row.net, row.entries
        ));
    }
    lines.join("\n")
}
