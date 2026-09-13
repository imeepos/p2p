//! llm-share offer 命令面：publish（组装 + 节点身份签名 + 原子落盘）/
//! show（当前生效声明 + 剩余 TTL 与生效状态）/ unpublish（撤销声明 = 停借）。

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
    /// 撤销当前能力声明（删除 offer.json，借出 serve 随之不再装配）
    Unpublish(UnpublishArgs),
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

#[derive(Args)]
pub struct UnpublishArgs {
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
        OfferCommand::Unpublish(args) => unpublish_cmd(args),
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

/// 撤销能力声明 = 删除 offer.json（serve 装配读不到 live 信封即不装配借出）。
/// 先读后删：报告被撤销声明的快照（含撤销时的生效状态）；无声明显式报错，
/// 与 show 同口径。删除即生效，节点侧下次装配面（重启）自然不再借出。
fn unpublish_cmd(args: UnpublishArgs) -> CliResult<()> {
    let file = offer::path(&args.data_dir);
    if !file.exists() {
        return Err(runtime_err(format!(
            "暂无能力声明（{}）：无可撤销的 offer",
            file.display()
        )));
    }
    let report = offer::show(&args.data_dir, now_secs()).map_err(runtime_err)?;
    std::fs::remove_file(&file)
        .map_err(|e| runtime_err(format!("能力声明撤销失败（{}）: {e}", file.display())))?;
    output::emit(
        args.json,
        &report,
        &format!("{}\nrevoked=true", render(&report.offer, Some(&report))),
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_identity::Keypair;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn scratch_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "cli-offer-unpublish-{tag}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 经真实 publish 路径造 live 声明（节点身份种子 = 32 字节原始文件）。
    fn publish_live_offer(data_dir: &PathBuf) {
        let dir_str = data_dir.to_str().unwrap();
        let seed = seed_path(dir_str).unwrap();
        if let Some(seed_dir) = seed.parent() {
            std::fs::create_dir_all(seed_dir).unwrap();
        }
        std::fs::write(&seed, Keypair::generate().to_seed_bytes()).unwrap();
        let params = OfferParams {
            models: vec!["test-model".into()],
            spare: vec!["test-model=1000".into()],
            period_ends: "2030-01-01".into(),
            max_per_req: vec![],
            rpm: 10,
            concurrency: 1,
            ttl_secs: 3600,
            retention: Some("none".into()),
        };
        offer::publish(&seed, dir_str, &params, now_secs()).unwrap();
    }

    #[test]
    fn unpublish_removes_live_offer_file() {
        let dir = scratch_dir("ok");
        publish_live_offer(&dir);
        let file = offer::path(dir.to_str().unwrap());
        assert!(file.exists());
        unpublish_cmd(UnpublishArgs {
            json: true,
            data_dir: dir.to_string_lossy().into_owned(),
        })
        .unwrap();
        assert!(
            !file.exists(),
            "撤销后 offer.json 必须移除（serve 不再装配借出）"
        );
    }

    #[test]
    fn unpublish_without_offer_errors_explicitly() {
        let dir = scratch_dir("missing");
        let err = unpublish_cmd(UnpublishArgs {
            json: false,
            data_dir: dir.to_string_lossy().into_owned(),
        })
        .unwrap_err();
        assert!(err.to_string().contains("暂无能力声明"), "实际: {err}");
    }
}
