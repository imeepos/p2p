//! acp share 子命令（acp-share 设计 §6）：create/list/revoke 直读台账
//! （<data-dir>/acp-shares.json），与 agent admin HTTP 同源同语义。
//! create 输出 JSON {share_id, token, link, ...}（设计 §6 冻结契约）；
//! link 要素 peer 取 agent 节点身份（<data-dir>/identity/key.seed），
//! addr 经 --addr 显式登记（可重复）。

use acp_common::policy::Scope;
use acp_common::{
    build_share_link, generate_token, unix_now, ShareEntry, ShareSpec, SHARE_FINGERPRINT_PREFIX,
};
use clap::{Args, Subcommand};

use super::store;
use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;
use crate::output;

mod report;
use report::{render_list, ShareCreateReport, ShareListEntry, ShareListReport, ShareRevokeReport};

/// share 子命令组（设计 §6 CLI 对等面）。
#[derive(Args)]
pub struct ShareArgs {
    #[command(subcommand)]
    pub command: ShareCommand,
}

#[derive(Subcommand)]
pub enum ShareCommand {
    /// 创建分享（stdout JSON 含 share_id/token/link）
    Create(ShareCreateArgs),
    /// 列出全部分享（脱敏：无 token 原文/哈希）
    List(ShareListArgs),
    /// 撤销分享（级联删除 share 来源策略条目）
    Revoke(ShareRevokeArgs),
}

#[derive(Args)]
pub struct ShareCreateArgs {
    /// 工作区边界：sandbox（默认）或 workspace（见退出码说明）
    #[arg(long, value_enum, default_value = "sandbox")]
    pub scope: super::ScopeArg,
    /// 有效期秒数（必填，创建时算出 expires_at）
    #[arg(long)]
    pub ttl_secs: u64,
    /// 激活次数上限（默认 1；一次性 = 激活次数）
    #[arg(long, default_value_t = 1)]
    pub max_activations: u32,
    /// mcpServers 白名单（可重复）
    #[arg(long = "allow-mcp")]
    pub allow_mcp: Vec<String>,
    /// request_permission 中 ask 的路由
    #[arg(long, value_enum, default_value = "remote_gui")]
    pub ask_route: super::AskRouteArg,
    /// 备注（可选）
    #[arg(long)]
    pub note: Option<String>,
    /// 链接候选地址（可重复；agent 的 QUIC/TCP/中继地址）
    #[arg(long = "addr")]
    pub addrs: Vec<String>,
    /// ACP 桥数据目录（台账/策略表/agent 身份所在）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Args)]
pub struct ShareListArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// ACP 桥数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Args)]
pub struct ShareRevokeArgs {
    /// 分享 ID（创建输出中的 share_id）
    pub share_id: String,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// ACP 桥数据目录
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

fn create_cmd(args: ShareCreateArgs) -> CliResult<()> {
    let report = create_share(&args)?;
    // 设计 §6：create 恒输出 JSON（token 原文只出现在本次 stdout 与链接里）。
    output::emit(true, &report, "")
}

fn list_cmd(args: ShareListArgs) -> CliResult<()> {
    let entries = list_entries(&args.data_dir)?;
    let text = render_list(&entries);
    output::emit(args.json, &ShareListReport { shares: entries }, &text)
}

fn revoke_cmd(args: ShareRevokeArgs) -> CliResult<()> {
    let report = revoke_share(&args)?;
    let text = format!(
        "已撤销分享 share_id={}（级联删除策略条目：{}；撤销前状态：{}）",
        report.share_id,
        if report.policy_removed { "是" } else { "否" },
        if report.already_revoked {
            "已撤销"
        } else {
            "有效"
        },
    );
    output::emit(args.json, &report, &text)
}

/// create 主流程：校验 → 生成 token（只落哈希）→ 原子写台账 → 组装链接。
pub(crate) fn create_share(args: &ShareCreateArgs) -> CliResult<ShareCreateReport> {
    if args.ttl_secs == 0 {
        return Err(CliError::Runtime("--ttl-secs 必须为正整数".to_owned()));
    }
    if args.max_activations == 0 {
        return Err(CliError::Runtime(
            "--max-activations 必须为正整数（一次性 = 1）".to_owned(),
        ));
    }
    let scope = Scope::from(args.scope);
    if scope == Scope::Workspace {
        return Err(CliError::Runtime(
            "scope=workspace 创建被拒绝：CLI 直读台账无法确认 agent 已配 --workspace-dir（设计 §11-Q5 fail-closed），请经 agent admin HTTP/GUI 创建"
                .to_owned(),
        ));
    }
    let allow_mcp = super::dedupe_mcp_names(&args.allow_mcp)?;
    let peer = agent_peer_id(&args.data_dir)?;
    let now = unix_now();
    let token = generate_token();
    let entry = ShareEntry::new(
        ShareSpec {
            scope,
            allow_mcp,
            ask_route: args.ask_route.into(),
            max_activations: args.max_activations,
            note: args.note.clone().unwrap_or_default(),
            ttl_secs: args.ttl_secs,
        },
        &token,
        now,
        uuid::Uuid::new_v4(),
    );
    let path = store::shares_path(&args.data_dir);
    let mut ledger = store::load_shares_or_empty(&path)?;
    ledger.insert(entry.clone());
    store::save_shares(&path, &ledger)?;
    let link = build_share_link(
        &peer,
        &args.addrs,
        &token,
        entry.expires_at_unix,
        &entry.share_id.to_string(),
    );
    Ok(ShareCreateReport {
        share_id: entry.share_id.to_string(),
        token,
        link,
        peer,
        addrs: args.addrs.clone(),
        scope: entry.scope,
        expires_at_unix: entry.expires_at_unix,
        created_at: entry.created_at,
    })
}

/// 链接 peer 要素：agent 节点身份（load-only，绝不代生成身份）。
fn agent_peer_id(data_dir: &str) -> CliResult<String> {
    let seed = acp_common::AcpPaths::new(data_dir)
        .root
        .join("identity")
        .join("key.seed");
    if !seed.exists() {
        return Err(CliError::Runtime(format!(
            "分享链接需要 agent 节点身份：{} 不存在；先启动一次 acp-agent 再创建分享",
            seed.display()
        )));
    }
    let keypair = p2p_identity::load_seed(&seed)
        .map_err(|e| CliError::Runtime(format!("agent 身份加载失败 {}: {e}", seed.display())))?;
    Ok(keypair.peer_id().to_string())
}

/// list 主流程：读台账（缺失视为空账）→ 脱敏条目（status 由时刻推导）。
pub(crate) fn list_entries(data_dir: &str) -> CliResult<Vec<ShareListEntry>> {
    let path = store::shares_path(data_dir);
    let ledger = store::load_shares_or_empty(&path)?;
    let now = unix_now();
    Ok(ledger
        .iter()
        .map(|(_, entry)| ShareListEntry {
            share_id: entry.share_id.to_string(),
            scope: entry.scope,
            allow_mcp: entry.allow_mcp.clone(),
            ask_route: entry.ask_route,
            note: entry.note.clone(),
            max_activations: entry.max_activations,
            activations: entry.activations,
            expires_at_unix: entry.expires_at_unix,
            revoked: entry.revoked,
            bound_peer: entry.bound_peer.clone(),
            created_at: entry.created_at.clone(),
            status: entry.status(now).to_owned(),
        })
        .collect())
}

/// revoke 主流程：置 revoked + 级联删除 share 来源策略条目（设计 §3）。
pub(crate) fn revoke_share(args: &ShareRevokeArgs) -> CliResult<ShareRevokeReport> {
    let path = store::shares_path(&args.data_dir);
    let mut ledger = store::load_shares_or_empty(&path)?;
    let Some(entry) = ledger.get(&args.share_id) else {
        return Err(CliError::Runtime(format!(
            "无此 share_id：{}",
            args.share_id
        )));
    };
    let already_revoked = entry.revoked;
    let bound_peer = entry.bound_peer.clone();
    let Some(slot) = ledger.get_mut(&args.share_id) else {
        return Err(CliError::Runtime(format!(
            "无此 share_id：{}",
            args.share_id
        )));
    };
    slot.revoked = true;
    store::save_shares(&path, &ledger)?;
    let policy_removed = match bound_peer {
        Some(peer) => drop_share_policy(&args.data_dir, &peer)?,
        None => false,
    };
    Ok(ShareRevokeReport {
        share_id: args.share_id.clone(),
        revoked: true,
        already_revoked,
        policy_removed,
    })
}

fn drop_share_policy(data_dir: &str, peer: &str) -> CliResult<bool> {
    let policy_path = store::policy_path(data_dir);
    let mut table = store::load_or_empty(&policy_path)?;
    let share_sourced = table
        .lookup(peer)
        .is_some_and(|p| p.fingerprint.starts_with(SHARE_FINGERPRINT_PREFIX));
    if !share_sourced {
        return Ok(false);
    }
    table.revoke(peer);
    store::save(&policy_path, &table)?;
    Ok(true)
}
