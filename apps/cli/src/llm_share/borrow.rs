//! llm-share borrow 命令面（F11/PR6）：借方一次性调用入口。逻辑面在
//! p2p-cli::llm_share::borrow，本层做 clap 参数映射、节点目录/引导配置装配
//! （节点身份与出借方 allowlist 登记同根）与双形态输出。拒绝路径报告照常
//! 输出（stdout 可采集），进程以退出码 1 显式失败。

use clap::Args;
use p2p_cli::llm_share::borrow::{self, BorrowParams};
use p2p_cli::llm_share::borrow_report::BorrowReport;

use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;
use crate::output;
use crate::paths::Paths;
use crate::store;

use super::runtime_err;

#[derive(Args)]
pub struct BorrowArgs {
    /// 出借方 PeerId（base58，32 字节）
    pub lender: String,
    /// 模型名（缺省 = 出借方声明内唯一模型，多个时列出可选项报错）
    #[arg(long)]
    pub model: Option<String>,
    /// 单条用户消息（与 --messages 二选一）
    #[arg(long)]
    pub prompt: Option<String>,
    /// OpenAI messages 数组 JSON（与 --prompt 二选一）
    #[arg(long)]
    pub messages: Option<String>,
    /// 单请求 max_tokens（缺省 = 出借方声明上限，未声明 256）
    #[arg(long)]
    pub max_tokens: Option<u64>,
    /// 出借方直连地址（ip/u端口 或 ip/t端口）；缺省经 rendezvous 查号
    #[arg(long = "addr")]
    pub addr: Option<String>,
    /// 代理调用全程超时秒
    #[arg(long, default_value_t = 90)]
    pub timeout_secs: u64,
    /// rendezvous 查号等待秒
    #[arg(long, default_value_t = 10)]
    pub discover_secs: u64,
    /// 幂等键（缺省生成 UUID v4；失败重试复用同值防双记账）
    #[arg(long)]
    pub req_id: Option<String>,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录（节点身份与流水同根）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

pub async fn borrow_cmd(args: BorrowArgs) -> CliResult<()> {
    let params = assemble(&args)?;
    let report = borrow::run(&params).await.map_err(runtime_err)?;
    output::emit(args.json, &report, &render(&report))?;
    if report.status == "rejected" {
        return Err(CliError::Runtime(format!(
            "出借方拒绝（{}）: {}",
            report.code.as_deref().unwrap_or("unknown"),
            report.message.as_deref().unwrap_or("")
        )));
    }
    Ok(())
}

/// 参数装配：bootstrap 取节点配置（rendezvous 查号同源），节点数据目录取
/// node_data_dir 派生（与 identity/offer 域同一身份种子）。
fn assemble(args: &BorrowArgs) -> CliResult<BorrowParams> {
    let paths = Paths::new(&args.data_dir);
    let cfg = store::load_config(&paths);
    Ok(BorrowParams {
        lender: args.lender.clone(),
        model: args.model.clone(),
        prompt: args.prompt.clone(),
        messages_json: args.messages.clone(),
        max_tokens: args.max_tokens,
        addr: args.addr.clone(),
        bootstrap: cfg.bootstrap.clone(),
        node_dir: paths
            .node_data_dir(Some(&cfg.data_dir))
            .display()
            .to_string(),
        ledger_dir: args.data_dir.clone(),
        timeout_secs: args.timeout_secs,
        discover_secs: args.discover_secs,
        req_id: args.req_id.clone(),
    })
}

fn render(report: &BorrowReport) -> String {
    let mut lines = vec![
        format!("status={}", report.status),
        format!("lender={}", report.lender),
        format!("model={}", report.model),
        format!("req_id={}", report.req_id),
        format!("period={}", report.period),
        format!("code={}", report.code.as_deref().unwrap_or("")),
        format!("message={}", report.message.as_deref().unwrap_or("")),
        format!("usage=input={},output={}", report.input, report.output),
        format!("estimated={}", report.estimated),
        format!("dispute_window_secs={}", report.dispute_window_secs),
        format!("sse_frames={}", report.sse_frames),
        format!("ledger={}", report.ledger_file),
        format!("appended={}", report.appended),
    ];
    if !report.receipt_file.is_empty() {
        lines.push(format!("receipt_file={}", report.receipt_file));
    }
    for event in &report.sse {
        lines.push(event.clone());
    }
    lines.join("\n")
}
