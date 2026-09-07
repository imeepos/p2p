//! share 测试共享设施（仅 cfg(test)）。

use std::sync::{Arc, RwLock as StdRwLock};

use acp_common::policy::PolicyTable;
use acp_common::{AskRoute, Scope, ShareLedger, ShareSpec};

use super::ShareService;
use crate::audit::CaptureAudit;
use crate::config::AgentConfig;

pub const NOW: u64 = 1_000_000;

pub fn tmp_dir(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!("acp-agent-share-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("tmp dir");
    dir.to_string_lossy().into_owned()
}

pub struct Rig {
    pub service: ShareService,
    pub cfg: AgentConfig,
    pub policy: Arc<StdRwLock<PolicyTable>>,
    pub audit: Arc<CaptureAudit>,
}

pub fn rig(tag: &str) -> Rig {
    let cfg = AgentConfig {
        data_dir: tmp_dir(tag),
        ..AgentConfig::default()
    };
    let audit = Arc::new(CaptureAudit::new());
    let policy = Arc::new(StdRwLock::new(PolicyTable::new()));
    let workspaces =
        std::sync::Arc::new(crate::workspaces::WorkspaceStore::open_for_config(&cfg).expect("ws"));
    let service =
        ShareService::open(&cfg, workspaces, policy.clone(), audit.clone()).expect("open");
    Rig {
        service,
        cfg,
        policy,
        audit,
    }
}

pub fn spec(scope: Scope, ttl: u64, max: u32) -> ShareSpec {
    ShareSpec {
        scope,
        workspace: None,
        allow_mcp: vec!["fs".to_owned()],
        ask_route: AskRoute::RemoteGui,
        max_activations: max,
        note: "nb".to_owned(),
        ttl_secs: ttl,
    }
}

pub fn ledger_on_disk(cfg: &AgentConfig) -> ShareLedger {
    ShareLedger::load(&cfg.paths().shares()).expect("ledger readable")
}

pub fn hand_ledger(cfg: &AgentConfig, entry: super::ShareEntry) {
    let mut ledger = ShareLedger::new();
    ledger.insert(entry);
    ledger.save(&cfg.paths().shares()).expect("hand ledger");
}
