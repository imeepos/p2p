//! llm-share receipt 命令面：指定收据文件离线 Ed25519 验签。
//! FAIL 时报告照常输出（stdout 可采集），进程以退出码 1 显式失败。

use clap::{Args, Subcommand};

use p2p_cli::llm_share::receipt::{self, ReceiptVerifyReport};

use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;
use crate::output;

use super::{runtime_err, seed_path};

#[derive(Subcommand)]
pub enum ReceiptCommand {
    /// 离线验签一份收据文件（--pubkey 缺省读本机节点身份公钥）
    Verify(VerifyArgs),
}

#[derive(Args)]
pub struct VerifyArgs {
    /// 收据文件路径（§5.1 wire JSON）
    pub path: String,
    /// 出借方公钥（base58，32 字节；缺省读本机节点身份，验签同时校验与 lender 绑定）
    #[arg(long)]
    pub pubkey: Option<String>,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录（缺省 --pubkey 时读本机身份种子）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

pub fn run(command: ReceiptCommand) -> CliResult<()> {
    match command {
        ReceiptCommand::Verify(args) => verify_cmd(args),
    }
}

fn verify_cmd(args: VerifyArgs) -> CliResult<()> {
    let pubkey = match &args.pubkey {
        Some(pubkey) => pubkey.clone(),
        None => local_pubkey(&args.data_dir)?,
    };
    let report =
        receipt::verify_file(std::path::Path::new(&args.path), &pubkey).map_err(runtime_err)?;
    output::emit(args.json, &report, &render(&report))?;
    if report.verdict != "PASS" {
        return Err(CliError::Runtime(format!(
            "收据验签 FAIL: {}",
            report.reason
        )));
    }
    Ok(())
}

/// 缺省公钥：本机节点身份（与 offer publish 签名同根）；读不到显式报错不静默。
fn local_pubkey(data_dir: &str) -> CliResult<String> {
    let seed = seed_path(data_dir)?;
    let keypair = p2p_identity::load_seed(&seed).map_err(|e| {
        CliError::Runtime(format!(
            "本机身份公钥不可用（{}）: {e}；先运行 p2pctl identity init 或显式传 --pubkey",
            seed.display()
        ))
    })?;
    Ok(bs58::encode(keypair.public()).into_string())
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
