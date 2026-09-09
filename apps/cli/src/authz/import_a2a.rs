//! p2pctl authz import a2a（authz-role-design §9）：读 <data-dir>/a2a-grants.json，
//! 有任一 grant 的 peer upsert 绑定 operator，幂等可重跑。grants 表原处保留不删
//! （双查分层：agent 级对象粒度防线仍在 acp-agent 侧生效，import 只补 peer 级）。
//! 文件缺失视为空簿（首用态）；authz 表读写失败整体失败上抛（§11 红线 2）。

use std::path::Path;

use clap::Args;
use serde::{Deserialize, Serialize};

use p2p_authz::{Authz, Decision, Permission, SystemClock};

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;
use crate::output;

use super::runtime_err;

/// grants 簿文件名（相对 data_dir；与 acp-agent src/a2a/grants.rs GRANTS_FILE 同名）。
const GRANTS_FILE: &str = "a2a-grants.json";
/// §9 映射目标角色：有任一 grant 即 operator。
const TARGET_ROLE: &str = "operator";
/// 绑定备注（重跑 kept 不翻新，备注只在首次写入时落）。
const BIND_NOTE: &str = "import a2a-grants";

/// `p2pctl authz import a2a` 参数面（子命令枚举收敛在 authz/mod.rs 统一入口）。
#[derive(Args)]
pub struct ImportA2aArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PeerOutcome {
    peer: String,
    /// bound = 本次写入 / kept = 已可 invoke 跳过 / invalid = peer 非法
    state: &'static str,
    detail: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    grants_file: String,
    grants_total: usize,
    peers_total: usize,
    bound: usize,
    kept: usize,
    invalid: usize,
    outcomes: Vec<PeerOutcome>,
}

pub fn run(args: ImportA2aArgs) -> CliResult<()> {
    let report = import_a2a(&args.data_dir).map_err(runtime_err)?;
    output::emit(args.json, &report, &render(&report))
}

/// 迁移主流程：读簿 → 去重 peer → 逐个确保 operator 绑定。返回 Err 即硬失败
/// （grants 簿损坏 / authz 表不可读写），CLI 以退出码 1 上抛。
pub fn import_a2a(data_dir: &str) -> Result<ImportReport, String> {
    let root = Path::new(data_dir);
    let grants_path = root.join(GRANTS_FILE);
    let entries = read_grants(&grants_path)?;
    let mut peers: Vec<String> = Vec::new();
    for entry in &entries {
        if !peers.contains(&entry.peer) {
            peers.push(entry.peer.clone());
        }
    }
    let authz = Authz::new(root, SystemClock);
    let mut report = ImportReport {
        grants_file: grants_path.display().to_string(),
        grants_total: entries.len(),
        peers_total: peers.len(),
        bound: 0,
        kept: 0,
        invalid: 0,
        outcomes: Vec::new(),
    };
    for peer in peers {
        match ensure_operator(&authz, &peer) {
            Ok(state) => {
                report.record(&state);
                report.outcomes.push(PeerOutcome {
                    peer,
                    state: state.as_str(),
                    detail: state.detail(),
                });
            }
            Err(detail) => {
                report.invalid += 1;
                report.outcomes.push(PeerOutcome {
                    peer,
                    state: "invalid",
                    detail,
                });
            }
        }
    }
    Ok(report)
}

impl ImportReport {
    fn record(&mut self, state: &PeerState) {
        match state {
            PeerState::Bound => self.bound += 1,
            PeerState::Kept => self.kept += 1,
        }
    }
}

enum PeerState {
    Bound,
    Kept,
}

impl PeerState {
    fn as_str(&self) -> &'static str {
        match self {
            PeerState::Bound => "bound",
            PeerState::Kept => "kept",
        }
    }

    fn detail(&self) -> String {
        match self {
            PeerState::Bound => format!("已绑定 {TARGET_ROLE}"),
            PeerState::Kept => "绑定已可 invoke，跳过（幂等）".into(),
        }
    }
}

/// 幂等单 peer：已可 invoke 即 kept（不翻新 granted_at/备注）；否则 upsert
/// operator（覆盖弱角色——grant 事实在册，operator 是 §9 映射目标）。
/// peer 校验失败属条目级数据问题：列报 invalid 并继续其余 peer。
fn ensure_operator(authz: &Authz<SystemClock>, peer: &str) -> Result<PeerState, String> {
    validate_peer(peer)?;
    match authz
        .check(peer, Permission::A2A_INVOKE)
        .map_err(|e| format!("authz 判定失败: {e}"))?
    {
        Decision::Allow => Ok(PeerState::Kept),
        Decision::Deny(_) => {
            authz
                .bind(peer, TARGET_ROLE, None, BIND_NOTE)
                .map_err(|e| format!("authz 写入失败: {e}"))?;
            Ok(PeerState::Bound)
        }
    }
}

/// peer 校验：与 p2p-cli::authz::validate_peer 同语义（base58 解码恰 32 字节），
/// apps/cli 已有 bs58 依赖，就地同实现避免为两行逻辑引 lib 内部接口。
fn validate_peer(peer_id: &str) -> Result<(), String> {
    let decoded = bs58::decode(peer_id)
        .into_vec()
        .map_err(|_| format!("PeerId 非法（不是合法 base58）：{peer_id}"))?;
    if decoded.len() != 32 {
        return Err(format!(
            "PeerId 非法（解码后应恰 32 字节，实得 {}）：{peer_id}",
            decoded.len()
        ));
    }
    Ok(())
}

/// 读 grants 簿：缺失 = 空簿（首用态）；损坏显式报错；version 不校验
/// （对齐 acp-agent GrantStore::open 容差）。原文件任何情况下不被改写。
fn read_grants(path: &Path) -> Result<Vec<GrantEntryRaw>, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{} read: {e}", path.display())),
    };
    let file: GrantsEnvelope =
        serde_json::from_slice(&bytes).map_err(|e| format!("{} parse: {e}", path.display()))?;
    Ok(file.grants)
}

#[derive(Deserialize)]
struct GrantsEnvelope {
    #[serde(default)]
    grants: Vec<GrantEntryRaw>,
}

/// 形状对齐 acp-agent GrantEntry（camelCase）。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrantEntryRaw {
    #[allow(dead_code)]
    agent_id: String,
    peer: String,
    #[allow(dead_code)]
    granted_at: u64,
}

fn render(report: &ImportReport) -> String {
    let mut lines = vec![
        format!(
            "import a2a: {}（grant {} 条 / peer {} 个）",
            report.grants_file, report.grants_total, report.peers_total
        ),
        format!(
            "bound={} kept={} invalid={}（grants 表原处保留，双查不删）",
            report.bound, report.kept, report.invalid
        ),
    ];
    for outcome in &report.outcomes {
        lines.push(format!(
            "  [{}] {} {}",
            outcome.state, outcome.peer, outcome.detail
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests;
