//! p2pctl authz import friends（Amended A-4）：好友簿全量 peer 中无 authz 绑定者
//! 绑 default_role，幂等可重跑（重跑零新增），落 authz.import.friends 审计；
//! 与 GUI/daemon 节点启动的自动回填同走 [run_backfill] 共享入口（等价语义）。
//! 好友簿读取走 p2p-chat 静态读（无节点装配）；authz 读写失败整体上抛（红线 2）。

use std::path::Path;

use clap::Args;
use serde::Serialize;

use p2p_authz::ImportFriendsReport;

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;
use crate::output;
use crate::paths::Paths;
use crate::store::load_config;

use super::runtime_err;

#[derive(Args)]
pub struct ImportFriendsArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录（读 <data-dir>/chat/friends.json）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportReport {
    role: String,
    scanned: usize,
    bound: usize,
    skipped: usize,
    disabled: bool,
}

/// 共享回填入口：好友簿 → peer 全集 → p2p-authz 幂等回填（命令面与 daemon
/// 启动回填同走此函数，保证两处等价）。
pub(crate) fn run_backfill(
    data_dir: &Path,
    default_role: &str,
) -> Result<ImportFriendsReport, String> {
    let friends = p2p_chat::friends_book(data_dir).map_err(|e| e.to_string())?;
    let peers: Vec<String> = friends.iter().map(|f| f.peer_id.clone()).collect();
    p2p_authz::import_friends(data_dir, &peers, default_role).map_err(|e| e.to_string())
}

/// daemon 节点启动装配回填（plan §0.5c）：成功升级后首次启动即补齐老好友绑定；
/// 失败降级告警不阻断启动（判定面默认拒绝兜底，重启可重跑）。观测沿 daemon
/// stderr 口径（p2pctl-daemon: 前缀），与 daemon 其余失败信号同源可 grep。
pub fn startup_backfill(data_dir: &Path) {
    let cfg = load_config(&Paths::new(&data_dir.to_string_lossy()));
    let role = cfg.authz_default_role;
    if role.is_empty() {
        return;
    }
    match run_backfill(data_dir, &role) {
        Ok(report) => eprintln!(
            "p2pctl-daemon: authz 好友存量回填完成 bound={} skipped={}",
            report.bound, report.skipped
        ),
        Err(e) => eprintln!("p2pctl-daemon: authz 好友存量回填失败，跳过（重启可重跑）: {e}"),
    }
}

pub fn run(args: ImportFriendsArgs) -> CliResult<()> {
    let cfg = load_config(&Paths::new(&args.data_dir));
    let filled = run_backfill(Path::new(&args.data_dir), &cfg.authz_default_role)
        .map_err(runtime_err)?;
    let report = ImportReport {
        role: cfg.authz_default_role,
        scanned: filled.scanned,
        bound: filled.bound,
        skipped: filled.skipped,
        disabled: filled.disabled,
    };
    output::emit(args.json, &report, &render(&report))
}

fn render(report: &ImportReport) -> String {
    if report.disabled {
        return format!(
            "authz.default_role 未配置（空串=禁用），扫描 {} 位好友，未做任何变更",
            report.scanned
        );
    }
    format!(
        "好友存量回填完成（default_role={}）：共 {} 位，绑定 {}，跳过 {}（幂等可重跑）",
        report.role, report.scanned, report.bound, report.skipped
    )
}
