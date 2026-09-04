//! llm-share 命令域（T21，idle-token-sharing-plan §6 末行）：出借方 allowlist、
//! 能力声明签发/查看、双边流水查询、收据离线验签。逻辑面在 p2p-cli::llm_share，
//! 本层只做 clap 参数映射与双形态输出（默认 key=value 文本，--json 结构化）。
//! 数据在 <data-dir>/llm-share/；声明签发与净差本机视角经节点身份种子（0600 标准）。

mod allow;
mod ledger;
mod offer;
mod receipt;

use std::path::PathBuf;

use clap::Subcommand;

use crate::error::{CliError, CliResult};
use crate::paths::Paths;
use crate::store;

#[derive(Subcommand)]
pub enum LlmShareCommand {
    /// allowlist：授予借方（upsert，可带模型白名单）
    Allow(allow::AllowArgs),
    /// allowlist：查看全部条目（BTreeMap 序）
    Allowlist(allow::ListArgs),
    /// allowlist：移除借方（不存在明确报错）
    Deny(allow::DenyArgs),
    /// 能力声明：publish 签名发布 / show 查看生效声明与剩余 TTL
    Offer {
        #[command(subcommand)]
        command: offer::OfferCommand,
    },
    /// 双边流水：list 明细 / balance 净差（按 lender+period 切分）
    Ledger {
        #[command(subcommand)]
        command: ledger::LedgerCommand,
    },
    /// 收据离线验签（PASS/FAIL）
    Receipt {
        #[command(subcommand)]
        command: receipt::ReceiptCommand,
    },
}

pub async fn run(command: LlmShareCommand) -> CliResult<()> {
    match command {
        LlmShareCommand::Allow(args) => allow::allow_cmd(args),
        LlmShareCommand::Allowlist(args) => allow::list_cmd(args),
        LlmShareCommand::Deny(args) => allow::deny_cmd(args),
        LlmShareCommand::Offer { command } => offer::run(command),
        LlmShareCommand::Ledger { command } => ledger::run(command),
        LlmShareCommand::Receipt { command } => receipt::run(command),
    }
}

/// 本机身份种子路径：与 node/identity 域同一派生（配置 dataDir，缺省 <data-dir>/p2p-data）。
pub(crate) fn seed_path(data_dir: &str) -> CliResult<PathBuf> {
    let paths = Paths::new(data_dir);
    let cfg = store::load_config(&paths);
    Ok(paths.node_data_dir(Some(&cfg.data_dir)).join("key.seed"))
}

/// 本机 PeerId（base58）：声明 peer 与流水净差的本机侧标识。
pub(crate) fn self_peer_id(data_dir: &str) -> CliResult<String> {
    let seed = seed_path(data_dir)?;
    let keypair = p2p_identity::load_seed(&seed)
        .map_err(|e| CliError::Runtime(format!("节点身份加载失败（{}）: {e}", seed.display())))?;
    Ok(keypair.peer_id().to_string())
}

/// 逻辑层 String 错误 → CLI 运行失败（退出码 1）。
pub(crate) fn runtime_err(e: String) -> CliError {
    CliError::Runtime(e)
}
