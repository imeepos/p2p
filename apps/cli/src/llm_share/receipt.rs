//! llm-share receipt 命令面：指定收据文件离线 Ed25519 验签。
//! FAIL 时报告照常输出（stdout 可采集），进程以退出码 1 显式失败。

use clap::{Args, Subcommand};

use p2p_cli::llm_share::receipt::{self, ReceiptVerifyReport};

use crate::error::{CliError, CliResult};
use crate::output;

use super::runtime_err;

#[derive(Subcommand)]
pub enum ReceiptCommand {
    /// 离线验签一份收据文件（出借方公钥取自其 offer 信封 pubkey 字段）
    Verify(VerifyArgs),
}

#[derive(Args)]
pub struct VerifyArgs {
    /// 收据文件路径（§5.1 wire JSON）
    pub path: String,
    /// 出借方公钥（base58，32 字节；验签同时校验与 lender PeerId 绑定）
    #[arg(long)]
    pub pubkey: String,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
}

pub fn run(command: ReceiptCommand) -> CliResult<()> {
    match command {
        ReceiptCommand::Verify(args) => verify_cmd(args),
    }
}

fn verify_cmd(args: VerifyArgs) -> CliResult<()> {
    let report = receipt::verify_file(std::path::Path::new(&args.path), &args.pubkey)
        .map_err(runtime_err)?;
    output::emit(args.json, &report, &render(&report))?;
    if report.verdict != "PASS" {
        return Err(CliError::Runtime(format!(
            "收据验签 FAIL: {}",
            report.reason
        )));
    }
    Ok(())
}

fn render(report: &ReceiptVerifyReport) -> String {
    format!(
        "verdict={}\nreason={}\nreq_id={}\nperiod={}\nlender={}\nborrower={}\nmodel={}\nusage=input={},output={}\nestimated={}\nts={}",
        report.verdict,
        report.reason,
        report.req_id,
        report.period,
        report.lender,
        report.borrower,
        report.model,
        report.input,
        report.output,
        report.estimated,
        report.ts
    )
}
