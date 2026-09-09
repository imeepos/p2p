//! p2pctl authz import acp（authz-role-design §9 迁移）：读 ACP 策略表落盘
//! （<data-dir>/acp-policy.json，与 p2pctl acp / 桥同一路径约定），按 scope
//! 映射写绑定：sandbox→guest、workspace→operator、Owner 条目跳过（loopback
//! 不进 authz，红线 1）。幂等：已有绑定的 peer 一律跳过（不覆盖人工改绑，
//! 重跑零副作用）；策略表文件只读，fingerprint/allow_mcp/workspace 等
//! 执行细节字段原处不动（对象粒度仍归策略表，§8）。

use std::path::Path;

use acp_common::Scope;
use clap::Args;
use serde::Serialize;

use p2p_authz::{store as authz_store, Authz, SystemClock};

use crate::acp::store as acp_store;
use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;

use super::runtime_err;

/// `authz import acp` 参数（挂 mod.rs 的 import 来源枢纽之下）。
#[derive(Args)]
pub struct AcpArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    pub json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: String,
}

pub fn run(args: AcpArgs) -> CliResult<()> {
    let report = import_acp(&args.data_dir).map_err(runtime_err)?;
    crate::output::emit(args.json, &report, &render(&report))
}

/// 迁移报告（camelCase 供 --json）。
#[derive(Debug, Serialize)]
struct ImportAcpReport {
    scanned: usize,
    bound_guest: usize,
    bound_operator: usize,
    skipped_owner: usize,
    skipped_bound: usize,
    skipped_invalid_peer: usize,
}

fn import_acp(data_dir: &str) -> Result<ImportAcpReport, String> {
    let table =
        acp_store::load_or_empty(&acp_store::policy_path(data_dir)).map_err(|e| e.to_string())?;
    let existing: Vec<String> = authz_store::load_bindings(Path::new(data_dir))
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|binding| binding.peer_id)
        .collect();
    let authz = Authz::new(Path::new(data_dir), SystemClock);
    let mut report = ImportAcpReport {
        scanned: 0,
        bound_guest: 0,
        bound_operator: 0,
        skipped_owner: 0,
        skipped_bound: 0,
        skipped_invalid_peer: 0,
    };
    for (peer, policy) in table.peers() {
        report.scanned += 1;
        let Some(role) = binding_role(policy.scope) else {
            report.skipped_owner += 1;
            continue;
        };
        if !valid_peer(peer) {
            report.skipped_invalid_peer += 1;
            continue;
        }
        if existing.iter().any(|bound| bound == peer) {
            report.skipped_bound += 1;
            continue;
        }
        authz
            .bind(peer, role, None, "import acp")
            .map_err(|e| e.to_string())?;
        if role == "guest" {
            report.bound_guest += 1;
        } else {
            report.bound_operator += 1;
        }
    }
    Ok(report)
}

/// §9 映射：sandbox→guest / workspace→operator；Owner 条目跳过（红线 1）。
fn binding_role(scope: Scope) -> Option<&'static str> {
    match scope {
        Scope::Sandbox => Some("guest"),
        Scope::Workspace => Some("operator"),
        Scope::Owner => None,
    }
}

/// peer 防御校验：base58 解码恰 32 字节（与 p2p-cli validate_peer 同语义）。
fn valid_peer(peer: &str) -> bool {
    bs58::decode(peer)
        .into_vec()
        .map(|raw| raw.len() == 32)
        .unwrap_or(false)
}

fn render(report: &ImportAcpReport) -> String {
    format!(
        "已扫描 {} 条策略条目：sandbox→guest {} 条、workspace→operator {} 条；\
         跳过 Owner {} 条、已有绑定 {} 条、非法 peer {} 条",
        report.scanned,
        report.bound_guest,
        report.bound_operator,
        report.skipped_owner,
        report.skipped_bound,
        report.skipped_invalid_peer,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_common::PolicyTable;
    use acp_common::{AskRoute, PeerPolicy};

    fn peer_id(seed: u8) -> String {
        bs58::encode([seed; 32]).into_string()
    }

    fn entry(scope: Scope) -> PeerPolicy {
        PeerPolicy {
            scope,
            allow_mcp: vec!["fs".to_owned()],
            ask_route: AskRoute::RemoteGui,
            note: String::new(),
            granted_at: "2026-01-01T00:00:00Z".to_owned(),
            fingerprint: "tofu-fingerprint".to_owned(),
            workspace: None,
        }
    }

    fn seed_policy(dir: &Path, peers: &[(String, Scope)]) -> Vec<u8> {
        let mut table = PolicyTable::new();
        for (peer, scope) in peers {
            table.grant(peer.clone(), entry(*scope));
        }
        let path = acp_store::policy_path(dir.to_str().expect("utf8"));
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        table.save(&path).expect("save policy");
        std::fs::read(&path).expect("read back")
    }

    fn tmp_dir(tag: &str) -> String {
        let dir = std::env::temp_dir().join(format!("authz-import-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tmp dir");
        dir.to_str().expect("utf8").to_owned()
    }

    fn bindings(dir: &str) -> Vec<(String, String)> {
        authz_store::load_bindings(Path::new(dir))
            .expect("bindings readable")
            .into_iter()
            .map(|b| (b.peer_id, b.role_id))
            .collect()
    }

    #[test]
    fn maps_scopes_and_skips_owner_entries() {
        let dir = tmp_dir("map");
        let (sandbox, workspace, owner) = (peer_id(1), peer_id(2), peer_id(3));
        seed_policy(
            Path::new(&dir),
            &[
                (sandbox.clone(), Scope::Sandbox),
                (workspace.clone(), Scope::Workspace),
                (owner.clone(), Scope::Owner),
            ],
        );
        let report = import_acp(&dir).expect("import");
        assert_eq!(report.scanned, 3);
        assert_eq!(report.bound_guest, 1);
        assert_eq!(report.bound_operator, 1);
        assert_eq!(report.skipped_owner, 1);
        let mut got = bindings(&dir);
        got.sort();
        assert_eq!(
            got,
            vec![
                (sandbox, "guest".to_owned()),
                (workspace, "operator".to_owned())
            ],
            "owner entry must not enter authz (red line 1)"
        );
    }

    #[test]
    fn rerun_is_idempotent_and_never_overwrites_manual_rebind() {
        let dir = tmp_dir("idem");
        let (sandbox, workspace) = (peer_id(4), peer_id(5));
        seed_policy(
            Path::new(&dir),
            &[
                (sandbox.clone(), Scope::Sandbox),
                (workspace.clone(), Scope::Workspace),
            ],
        );
        import_acp(&dir).expect("first import");
        let first = bindings(&dir);
        let report = import_acp(&dir).expect("second import");
        assert_eq!(
            report.skipped_bound, 2,
            "re-run must skip existing bindings"
        );
        assert_eq!(bindings(&dir), first, "re-run must not touch any binding");
        // 人工改绑后重跑：导入不得覆盖更严/更宽的人工决定。
        let authz = Authz::new(Path::new(&dir), SystemClock);
        authz
            .bind(&sandbox, "operator", None, "manual")
            .expect("rebind");
        import_acp(&dir).expect("third import");
        let rebound = bindings(&dir)
            .into_iter()
            .find(|(peer, _)| peer == &sandbox)
            .expect("binding exists");
        assert_eq!(
            rebound.1, "operator",
            "manual rebind must survive re-import"
        );
    }

    #[test]
    fn policy_file_stays_byte_identical_after_import() {
        let dir = tmp_dir("untouched");
        let peer = peer_id(6);
        let before = seed_policy(Path::new(&dir), &[(peer, Scope::Sandbox)]);
        import_acp(&dir).expect("import");
        let after = std::fs::read(acp_store::policy_path(&dir)).expect("read after");
        assert_eq!(before, after, "import must be read-only on the policy file");
    }

    #[test]
    fn invalid_peer_ids_are_counted_not_fatal() {
        let dir = tmp_dir("invalid");
        seed_policy(
            Path::new(&dir),
            &[
                (peer_id(7), Scope::Sandbox),
                ("not-a-peer".to_owned(), Scope::Sandbox),
            ],
        );
        let report = import_acp(&dir).expect("import");
        assert_eq!(report.skipped_invalid_peer, 1);
        assert_eq!(report.bound_guest, 1);
    }
}
