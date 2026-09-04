//! repair-helper 装配：stdio 模式（T21/T23 存量）/ p2p 端点（T26）/ mint-ticket。
//!
//! p2p 模式（默认 serve 子命令）：起底座节点（身份密钥经 --data-dir 参数化）、
//! 向 rendezvous 注册（--bootstrap 接线）、受理 /repair/mcp/1 入站流（票据校验
//! 前置，--platform-pubkey 注入平台公钥）；审计 JSONL 落盘（--audit-file）。

use clap::{Parser, Subcommand};
use p2p::Node;
use p2p_identity::Keypair;
use repair_enforce::approval::Approver;
use repair_enforce::whitelist::ShellWhitelist;
use repair_helper::{
    audit::{self, AuditSink},
    enforce::Enforcement,
    jail::{split_roots, PathJail},
    p2p::{Endpoint, InboundPeers},
    ticket::{mint, parse_peer_id, TicketLedger, TicketVerifier},
    tools, Host,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::BufReader;
use tokio::sync::watch;

#[derive(Debug, Parser)]
#[command(
    name = "repair-helper",
    about = "远程支持临时 MCP 宿主（p2p 端点 + ticket + 审计）"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// 起 p2p 端点：注册到 rendezvous 并受理 /repair/mcp/1 入站流（票据校验前置）。
    Serve(ServeArgs),
    /// 铸造工单票据并立即自检（ticket 串走 stdout，自检信息走 stderr）。
    MintTicket(MintArgs),
}

#[derive(Debug, clap::Args)]
struct ServeArgs {
    /// rendezvous bootstrap 地址（可重复）；空则仅局域网可拨。
    #[arg(long, action = clap::ArgAction::Append)]
    bootstrap: Vec<String>,
    /// 身份数据目录（identity seed 持久化，重启身份不变）。
    #[arg(long, default_value = "./p2p-data")]
    data_dir: PathBuf,
    /// 平台 ed25519 公钥（hex 64 字符），票据签名验真参数。
    #[arg(long)]
    platform_pubkey: String,
    /// 审计 JSONL 落盘路径（启动截断）。
    #[arg(long, default_value = "./repair-audit.jsonl")]
    audit_file: PathBuf,
}

#[derive(Debug, clap::Args)]
struct MintArgs {
    /// 平台签发密钥种子文件（32 字节）。
    #[arg(long)]
    key: PathBuf,
    /// helper 对端 PeerId（base58）。
    #[arg(long)]
    helper_peer: String,
    /// bridge 对端 PeerId（base58）。
    #[arg(long)]
    bridge_peer: String,
    /// scope：diag | fix。
    #[arg(long)]
    scope: String,
    /// 有效期（秒）。
    #[arg(long)]
    ttl: u64,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let report = p2p_log::init(p2p_log::LogConfig::default());
    if let Some(fallback) = report.fallback {
        tracing::warn!(%fallback, "repair-helper logging fallback");
    }
    match Cli::parse().command {
        Some(Command::Serve(args)) => p2p_serve(args).await,
        Some(Command::MintTicket(args)) => mint_ticket(args),
        None => stdio_serve().await,
    }
}

/// p2p 端点模式：节点（身份参数化）+ rendezvous 注册 + 票据门 + 审计落盘。
async fn p2p_serve(args: ServeArgs) -> std::io::Result<()> {
    let pubkey = parse_pubkey_hex(&args.platform_pubkey)?;
    let jail = build_jail()?;
    let audit = AuditSink::with_file(&args.audit_file)?;
    let ledger = TicketLedger::default();
    let inbound = InboundPeers::default();
    let endpoint = Endpoint::new(
        TicketVerifier::new(pubkey, ledger),
        inbound.clone(),
        jail,
        audit,
        shell_whitelist(),
        shell_clock(),
        shell_approver(),
    )?;
    let node = Node::builder()
        .data_dir(args.data_dir.clone())
        .bootstrap(args.bootstrap.clone())
        .mdns(false)
        .build()
        .await
        .map_err(|e| {
            tracing::error!(%e, "repair-helper node build failed");
            std::io::Error::other(e)
        })?;
    node.set_gate(inbound.gate());
    node.handle_protocol(Arc::new(endpoint));
    tracing::info!(
        peer = %node.local_peer_id(),
        addrs = ?node.listen_addrs(),
        "repair-helper p2p endpoint ready (ticket required for inbound)"
    );
    eprintln!(
        "repair-helper p2p endpoint ready: peer {}",
        node.local_peer_id()
    );
    tokio::signal::ctrl_c().await?;
    node.shutdown();
    tracing::info!("repair-helper stopped; ticket ledger burned on exit");
    Ok(())
}

/// mint-ticket：读平台种子 -> 铸造 -> 立即自检 verify 通过才输出。
fn mint_ticket(args: MintArgs) -> std::io::Result<()> {
    let seed_bytes = std::fs::read(&args.key)?;
    let seed: [u8; 32] = seed_bytes.try_into().map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "key seed file must be 32 bytes",
        )
    })?;
    let key = Keypair::from_seed(&seed);
    let now = audit::now_unix_ms() / 1000;
    let ticket_id = format!("ticket-{now}");
    let ticket = mint(
        &key,
        &ticket_id,
        &args.helper_peer,
        &args.bridge_peer,
        &args.scope,
        args.ttl,
        now,
    )
    .map_err(|e| std::io::Error::other(e.to_string()))?;
    // 自检：mint 后立即 verify 必须通过（同密钥公钥 + 同 bridge_peer 作入流对端）。
    let inbound =
        parse_peer_id(&args.bridge_peer).map_err(|e| std::io::Error::other(e.to_string()))?;
    let payload = TicketVerifier::new(key.public(), TicketLedger::default())
        .verify(&ticket, &inbound, now)
        .map_err(|e| std::io::Error::other(format!("mint self-check failed: {e}")))?;
    eprintln!(
        "ticket id {} scope {} exp {} self-check ok",
        payload.ticket_id, payload.scope, payload.exp
    );
    println!("{ticket}");
    Ok(())
}

/// stdio 模式（T21/T23 存量）：本地临时 MCP server，不经票据。
async fn stdio_serve() -> std::io::Result<()> {
    let jail = build_jail()?;
    let enforcement = Enforcement::new(repair_enforce::Scope::Diag, shell_whitelist());
    let shell = tools::shell_exec::ShellExec::new(
        jail.clone(),
        enforcement.clone(),
        shell_clock(),
        shell_approver(),
    );
    let audit = AuditSink::default();
    let registry = tools::helper_registry(jail, shell, audit.clone());
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    Host::guarded(registry, enforcement, audit)
        .serve(
            BufReader::new(tokio::io::stdin()),
            tokio::io::stdout(),
            shutdown_rx,
        )
        .await
}

/// 装配白名单：T24 的 repair_enforce::builtin()（23 条 playbook 命令并集）。
/// 全仓唯一装配入口——两处形态（stdio/p2p）共用，防零散接线再退化为 empty。
fn shell_whitelist() -> ShellWhitelist {
    repair_enforce::builtin()
}

/// 审批墙钟（WallClock：真实 60s 超时即拒）与空队列审批通道
/// （无人应答即超时拒绝；行式/托盘通道由部署面接线，见 tools::approval）。
fn shell_clock() -> Arc<dyn repair_enforce::approval::Clock + Send + Sync> {
    Arc::new(repair_helper::tools::approval::WallClock::new())
}

fn shell_approver() -> Arc<Mutex<Box<dyn Approver + Send>>> {
    Arc::new(Mutex::new(
        Box::new(repair_helper::tools::approval::QueueApprover::new()) as Box<dyn Approver + Send>,
    ))
}

/// 授权根：REPAIR_ROOTS（: 分隔）显式配置，缺省临时演示根。
/// 失败的根配置必须显式报错退出，禁止静默降级。
fn build_jail() -> std::io::Result<PathJail> {
    let jail = match std::env::var("REPAIR_ROOTS") {
        Ok(raw) if !raw.trim().is_empty() => PathJail::from_roots(split_roots(&raw)),
        _ => PathJail::demo(),
    };
    jail.map_err(|e| {
        tracing::error!(%e, "repair-helper jail init failed");
        std::io::Error::other(e)
    })
}

/// hex 64 字符 -> ed25519 公钥 32 字节；非法输入显式报错。
fn parse_pubkey_hex(s: &str) -> std::io::Result<[u8; 32]> {
    let compact: Vec<u8> = s
        .strip_prefix("0x")
        .unwrap_or(s)
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    if compact.len() != 64 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "platform public key must be 64 hex chars",
        ));
    }
    let mut out = [0u8; 32];
    for (i, pair) in compact.chunks(2).enumerate() {
        let hi = hex_val(pair[0]).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "public key not hex")
        })?;
        let lo = hex_val(pair[1]).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "public key not hex")
        })?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 装配回归防线：main 装配白名单来自 T24 builtin，退化为 empty 会拒绝
    /// 一切 shell_exec（T23b2 立卡缘由），此处机械拦截再退化。
    #[test]
    fn assembly_whitelist_is_builtin_non_empty() {
        let w = shell_whitelist();
        assert!(
            !w.rules().is_empty(),
            "装配白名单为空：shell_exec 将拒绝一切"
        );
        assert!(
            w.find(&["netsh".to_string()]).is_some(),
            "builtin 应含 T24 playbook 表内程序（netsh）"
        );
    }

    /// 已知条目参数模式仍放行（接线后再退化即红）。
    #[test]
    fn builtin_allows_known_playbook_command() {
        let w = shell_whitelist();
        assert!(w.is_allowed(&[
            "netsh".to_string(),
            "winhttp".to_string(),
            "show".to_string(),
            "proxy".to_string(),
        ]));
    }
}
