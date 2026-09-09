//! p2pctl authz import llm-share：旧 allowlist 表 → authz 绑定迁移（§9 映射）。
//! <data-dir>/llm-share/allowlist.json 每个借方条目绑定内建角色 ally；
//! 已绑定 peer 一律跳过（更严者为准，不因 import 放宽）→ 重跑零增量幂等。
//! 过期时间随条目迁移（到期语义从严保留）；模型白名单留在原处不动；
//! allowlist 表只读归档不删除（回滚 = 撤下闸 1 authz 装配点恢复旧判定）。

use std::collections::HashMap;
use std::path::Path;

use clap::{Args, Subcommand};
use serde::Serialize;

use p2p_authz::{store as authz_store, Authz, SystemClock};
use p2p_cli::llm_share::allowlist;

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;
use crate::output;

use super::runtime_err;

/// 迁移目标角色（§9）：ally = operator + llm.borrow + repair.fix。
const TARGET_ROLE: &str = "ally";

#[derive(Subcommand)]
pub enum ImportCommand {
    /// llm-share/allowlist.json 借方条目 → 绑定内建角色 ally（幂等可重跑）
    LlmShare(LlmShareArgs),
}

#[derive(Args)]
pub struct LlmShareArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录（读 <data-dir>/llm-share/allowlist.json）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

/// import llm-share 报告（camelCase，--json 结构化输出）。
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub source: String,
    pub role: String,
    /// allowlist 表条目总数。
    pub entries: usize,
    /// 本次新建绑定数。
    pub imported: usize,
    /// 已有绑定跳过数（更严者为准，不因 import 放宽）。
    pub skipped: usize,
}

pub fn run(command: ImportCommand) -> CliResult<()> {
    match command {
        ImportCommand::LlmShare(args) => llm_share_cmd(args),
    }
}

fn llm_share_cmd(args: LlmShareArgs) -> CliResult<()> {
    let report = import(&args.data_dir).map_err(runtime_err)?;
    output::emit(args.json, &report, &render(&report))
}

fn render(report: &ImportReport) -> String {
    format!(
        "source={} role={} entries={} imported={} skipped={}（已绑定跳过，重跑零增量）",
        report.source, report.role, report.entries, report.imported, report.skipped
    )
}

/// 迁移主流程：读 allowlist → 已绑定集合 → 仅缺绑 peer 落 bind → 报告。
/// 逐条 bind 自带重读-合并-原子写（§6）；allowlist 表全程只读。
pub fn import(data_dir: &str) -> Result<ImportReport, String> {
    let list = allowlist::load_or_empty(&allowlist::path(data_dir))?;
    let existing: HashMap<String, String> = authz_store::load_bindings(Path::new(data_dir))
        .map_err(|e| format!("authz 绑定表读取失败: {e}"))?
        .into_iter()
        .map(|binding| (binding.peer_id, binding.role_id))
        .collect();
    let authz = Authz::new(Path::new(data_dir), SystemClock);
    let mut report = ImportReport {
        source: "llm-share/allowlist.json".into(),
        role: TARGET_ROLE.into(),
        entries: list.entries.len(),
        imported: 0,
        skipped: 0,
    };
    for (peer_id, entry) in &list.entries {
        if existing.contains_key(peer_id) {
            report.skipped += 1;
            continue;
        }
        authz.bind(peer_id, TARGET_ROLE, entry.expires_at, "import llm-share allowlist")
            .map_err(|e| format!("绑定 {peer_id} 失败: {e}"))?;
        report.imported += 1;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_authz::{store::load_bindings, Decision, DenyReason, Permission};
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("p2pctl-import-ls-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn write_allowlist(dir: &Path, grants: &[(&str, Option<u64>, &[&str])]) {
        let mut list = allowlist::AllowlistFile::new();
        for (peer, expires_at, models) in grants {
            list.upsert(peer, models.to_vec().iter().map(|s| s.to_string()).collect(), "", None, *expires_at, "2026-09-09T00:00:00Z");
        }
        allowlist::save(&allowlist::path(dir.to_str().expect("utf8")), &list).expect("save");
    }

    fn binding_role(dir: &Path, peer: &str) -> Option<String> {
        load_bindings(dir)
            .expect("load bindings")
            .into_iter()
            .find(|b| b.peer_id == peer)
            .map(|b| b.role_id)
    }

    fn check_borrow(dir: &Path, peer: &str) -> Decision {
        Authz::new(dir, SystemClock)
            .check(peer, Permission::LLM_BORROW)
            .expect("check")
    }

    #[test]
    fn imports_entries_as_ally_with_expiry_carried() {
        let dir = temp_dir("map");
        // peer-b 过期秒取 2100 年（未来）→ 放行；peer-c 取 1970 → 过期拒绝。
        let future = 4_102_444_800;
        write_allowlist(
            &dir,
            &[("peer-a", None, &["gpt-4o"]), ("peer-b", Some(future), &[]), ("peer-c", Some(1_000), &[])],
        );
        let report = import(dir.to_str().expect("utf8")).expect("import");
        assert_eq!(report.entries, 3);
        assert_eq!(report.imported, 3);
        assert_eq!(report.skipped, 0);
        assert_eq!(binding_role(&dir, "peer-a").as_deref(), Some("ally"));
        let bound_b = load_bindings(&dir)
            .expect("load")
            .into_iter()
            .find(|b| b.peer_id == "peer-b")
            .expect("peer-b");
        assert_eq!(bound_b.expires_at, Some(future), "过期语义须随条目迁移");
        assert_eq!(check_borrow(&dir, "peer-a"), Decision::Allow);
        assert_eq!(check_borrow(&dir, "peer-b"), Decision::Allow);
        assert_eq!(
            check_borrow(&dir, "peer-c"),
            Decision::Deny(DenyReason::Expired),
            "条目过期语义保留（偏严不偏宽）"
        );
    }

    #[test]
    fn rerun_is_zero_increment_and_file_unchanged() {
        let dir = temp_dir("idem");
        write_allowlist(&dir, &[("peer-a", None, &["gpt-4o"])]);
        let first = import(dir.to_str().expect("utf8")).expect("first");
        assert_eq!(first.imported, 1);
        let path = dir.join("authz").join("bindings.json");
        let before = fs::read(&path).expect("read bindings");
        let second = import(dir.to_str().expect("utf8")).expect("second");
        assert_eq!(second.imported, 0, "重跑不得新增绑定");
        assert_eq!(second.skipped, 1);
        assert_eq!(fs::read(&path).expect("read bindings"), before, "重跑零增量：落盘态逐字节不变");
    }

    #[test]
    fn existing_binding_not_loosened_by_import() {
        let dir = temp_dir("strict");
        write_allowlist(&dir, &[("peer-a", None, &[])]);
        Authz::new(&dir, SystemClock)
            .bind("peer-a", "operator", None, "pre-existing")
            .expect("pre bind");
        let report = import(dir.to_str().expect("utf8")).expect("import");
        assert_eq!(report.skipped, 1);
        assert_eq!(report.imported, 0);
        assert_eq!(binding_role(&dir, "peer-a").as_deref(), Some("operator"), "既有绑定不被覆盖");
        assert_eq!(
            check_borrow(&dir, "peer-a"),
            Decision::Deny(DenyReason::MissingPerm),
            "operator 无 llm.borrow：更严者为准"
        );
    }

    #[test]
    fn missing_allowlist_is_zero_entry_success() {
        let dir = temp_dir("empty");
        let report = import(dir.to_str().expect("utf8")).expect("import");
        assert_eq!(report, ImportReport {
            source: "llm-share/allowlist.json".into(),
            role: "ally".into(),
            entries: 0,
            imported: 0,
            skipped: 0,
        });
    }

    #[test]
    fn allowlist_file_stays_readonly_archive() {
        let dir = temp_dir("archive");
        write_allowlist(&dir, &[("peer-a", None, &["gpt-4o", "claude-3"])]);
        let path = allowlist::path(dir.to_str().expect("utf8"));
        let before = fs::read(&path).expect("read allowlist");
        import(dir.to_str().expect("utf8")).expect("import");
        assert_eq!(fs::read(&path).expect("read allowlist"), before, "原表只读归档：内容不得变动");
        let list = allowlist::load_or_empty(&path).expect("reload");
        assert_eq!(
            list.entries["peer-a"].models,
            vec!["gpt-4o".to_owned(), "claude-3".to_owned()],
            "模型白名单留在原处不动"
        );
    }
}
