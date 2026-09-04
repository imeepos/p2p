//! acp 命令域（ACP5）：节点主人策略管理面 allow/deny/list（设计 §3/§6）。
//! 数据面复用 apps/acp-common（策略表 serde+原子存取、AcpPaths 路径派生），
//! 策略文件 <data-dir>/acp-policy.json，与 acp-agent 同一目录约定。
//! 授权语义：默认拒绝——表无条目即拒；allow=upsert（granted_at 每次刷新），
//! deny=删条目；本卡不做交互确认（TOFU 指纹显式传入，交互面归 GUI 波）。

pub mod render;
pub mod store;

#[cfg(test)]
mod tests;

use acp_common::policy::{AskRoute, PeerPolicy, Scope};
use clap::{Args, Subcommand};

use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;
use crate::output;

use render::{AllowReport, DenyReport, ListEntry};

/// acp 域命令面：节点主人策略管理（headless 管理面，不涉 GUI 与 cli-parity）。
#[derive(Subcommand)]
pub enum AcpCommand {
    /// 授予 peer（upsert：条目已存在则为更新并刷新 granted_at）
    Allow(AllowArgs),
    /// 撤销授权（删除条目；不存在明确报错不静默）
    Deny(DenyArgs),
    /// 列出全部授权条目
    List(ListArgs),
}

#[derive(Args)]
pub struct AllowArgs {
    /// 对端 PeerId（base58，32 字节）
    pub peer_id: String,
    /// 工作区边界：sandbox=每 peer 监狱（默认）/ workspace=锁定授权目录
    #[arg(long, value_enum, default_value = "sandbox")]
    pub scope: ScopeArg,
    /// mcpServers 白名单：按名引用 host 预定义服务（可重复）
    #[arg(long = "allow-mcp")]
    pub allow_mcp: Vec<String>,
    /// request_permission 中 ask 的路由
    #[arg(long, value_enum, default_value = "remote_gui")]
    pub ask_route: AskRouteArg,
    /// 备注（可选）
    #[arg(long)]
    pub note: Option<String>,
    /// TOFU 指纹（显式登记进策略表）
    #[arg(long)]
    pub fingerprint: Option<String>,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录（策略表在 <data-dir>/acp-policy.json）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Args)]
pub struct DenyArgs {
    /// 对端 PeerId（base58，32 字节）
    pub peer_id: String,
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
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

/// --scope 取值（设计 §12-Q2：默认 sandbox）。
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum ScopeArg {
    /// 每 peer 监狱 <root>/<peerId>/
    Sandbox,
    /// 锁定授权目录
    Workspace,
}

/// --ask-route 取值（设计 §12-Q3：默认 remote_gui）。
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum AskRouteArg {
    /// 路由到远程操作者 GUI
    #[value(name = "remote_gui")]
    RemoteGui,
    /// 路由给节点主人本机
    #[value(name = "owner_local")]
    OwnerLocal,
}

impl From<ScopeArg> for Scope {
    fn from(value: ScopeArg) -> Self {
        match value {
            ScopeArg::Sandbox => Scope::Sandbox,
            ScopeArg::Workspace => Scope::Workspace,
        }
    }
}

impl From<AskRouteArg> for AskRoute {
    fn from(value: AskRouteArg) -> Self {
        match value {
            AskRouteArg::RemoteGui => AskRoute::RemoteGui,
            AskRouteArg::OwnerLocal => AskRoute::OwnerLocal,
        }
    }
}

pub async fn run(command: AcpCommand) -> CliResult<()> {
    match command {
        AcpCommand::Allow(args) => allow_cmd(args),
        AcpCommand::Deny(args) => deny_cmd(args),
        AcpCommand::List(args) => list_cmd(args),
    }
}

fn allow_cmd(args: AllowArgs) -> CliResult<()> {
    let report = allow_policy(&args)?;
    let text = render::render_allow(&report);
    output::emit(args.json, &report, &text)
}

fn deny_cmd(args: DenyArgs) -> CliResult<()> {
    let report = deny_policy(&args)?;
    let text = render::render_deny(&report);
    output::emit(args.json, &report, &text)
}

fn list_cmd(args: ListArgs) -> CliResult<()> {
    let entries = list_entries(&args)?;
    let text = render::render_list(&entries);
    output::emit(args.json, &render::ListReport { peers: entries }, &text)
}

/// allow 主流程：校验 → 读表 → upsert → 原子写回。
fn allow_policy(args: &AllowArgs) -> CliResult<AllowReport> {
    validate_peer_id(&args.peer_id)?;
    let allow_mcp = dedupe_mcp_names(&args.allow_mcp)?;
    let path = store::policy_path(&args.data_dir);
    let mut table = store::load_or_empty(&path)?;
    let created = table.lookup(&args.peer_id).is_none();
    let granted_at = store::rfc3339_now();
    table.grant(
        args.peer_id.as_str(),
        PeerPolicy {
            scope: args.scope.into(),
            allow_mcp: allow_mcp.clone(),
            ask_route: args.ask_route.into(),
            note: args.note.clone().unwrap_or_default(),
            granted_at: granted_at.clone(),
            fingerprint: args.fingerprint.clone().unwrap_or_default(),
        },
    );
    store::save(&path, &table)?;
    Ok(AllowReport {
        created,
        peer_id: args.peer_id.clone(),
        scope: args.scope.into(),
        allow_mcp,
        ask_route: args.ask_route.into(),
        granted_at,
    })
}

/// deny 主流程：校验 → 读表 → 删条目 → 原子写回；不存在明确报错（退出码 1）。
fn deny_policy(args: &DenyArgs) -> CliResult<DenyReport> {
    validate_peer_id(&args.peer_id)?;
    let path = store::policy_path(&args.data_dir);
    let mut table = store::load_or_empty(&path)?;
    if !table.revoke(&args.peer_id) {
        return Err(CliError::Runtime(format!(
            "策略表中无该 peer 条目：{}（本就默认拒绝，无需 deny）",
            args.peer_id
        )));
    }
    store::save(&path, &table)?;
    Ok(DenyReport {
        removed: true,
        peer_id: args.peer_id.clone(),
    })
}

/// list 主流程：读表（缺失视为空表）→ 条目透出（BTreeMap 序，稳定输出）。
fn list_entries(args: &ListArgs) -> CliResult<Vec<ListEntry>> {
    let path = store::policy_path(&args.data_dir);
    let table = store::load_or_empty(&path)?;
    Ok(table
        .peers()
        .map(|(peer_id, policy)| ListEntry {
            peer_id: peer_id.to_owned(),
            scope: policy.scope,
            allow_mcp: policy.allow_mcp.clone(),
            ask_route: policy.ask_route,
            granted_at: policy.granted_at.clone(),
            fingerprint: policy.fingerprint.clone(),
            note: policy.note.clone(),
        })
        .collect())
}

/// 对齐项目既有 PeerId 校验（p2p-identity PeerId 语义 / p2p-chat parse_peer_id
/// 同规则：base58 解码后恰 32 字节；该解析为 pub(crate) 不可跨 crate 复用）。
fn validate_peer_id(peer_id: &str) -> CliResult<()> {
    let decoded = bs58::decode(peer_id)
        .into_vec()
        .map_err(|_| CliError::Runtime(format!("PeerId 非法（不是合法 base58）：{peer_id}")))?;
    if decoded.len() != 32 {
        return Err(CliError::Runtime(format!(
            "PeerId 非法（解码后应恰 32 字节，实得 {}）：{peer_id}",
            decoded.len()
        )));
    }
    Ok(())
}

/// mcpServers 白名单服务名：trim、去重保序、拒绝空名（剥离语义见设计 §6 MCP 行）。
fn dedupe_mcp_names(raw: &[String]) -> CliResult<Vec<String>> {
    let mut names: Vec<String> = Vec::with_capacity(raw.len());
    for value in raw {
        let name = value.trim();
        if name.is_empty() {
            return Err(CliError::Runtime(
                "--allow-mcp 服务名不能为空（白名单按名引用 host 预定义服务）".to_owned(),
            ));
        }
        if !names.iter().any(|known| known == name) {
            names.push(name.to_owned());
        }
    }
    Ok(names)
}
