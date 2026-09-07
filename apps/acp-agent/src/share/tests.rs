//! ShareService 测试：台账存取口径、只存哈希、存储故障 fail-closed、scope 约束。

use std::sync::{Arc, RwLock as StdRwLock};

use acp_common::policy::PolicyTable;
use acp_common::Scope;

use super::testutil::{ledger_on_disk, rig, spec, tmp_dir, NOW};
use super::*;
use crate::audit::CaptureAudit;
use crate::config::{AgentConfig, WorkspaceDef};

#[test]
fn open_missing_ledger_starts_empty_with_warn() {
    let r = rig("empty");
    assert_eq!(r.service.list().len(), 0);
}

#[test]
fn open_corrupt_ledger_refuses_start() {
    let cfg = AgentConfig {
        data_dir: tmp_dir("corrupt"),
        ..AgentConfig::default()
    };
    std::fs::write(cfg.paths().shares(), "not json").expect("write");
    let audit = Arc::new(CaptureAudit::new());
    let policy = Arc::new(StdRwLock::new(PolicyTable::new()));
    let err = match ShareService::open(
        &cfg,
        std::sync::Arc::new(crate::workspaces::WorkspaceStore::open_for_config(&cfg).unwrap()),
        policy,
        audit,
    ) {
        Err(err) => err,
        Ok(_) => panic!("损坏台账必须拒启"),
    };
    assert!(matches!(err, ShareStoreError::Corrupted(_)));
}

#[test]
fn create_persists_hash_never_plaintext() {
    let r = rig("hash");
    let created = r
        .service
        .create(spec(Scope::Sandbox, 3_600, 1), NOW)
        .expect("create");
    let raw = std::fs::read_to_string(r.cfg.paths().shares()).expect("read ledger");
    assert!(!raw.contains(&created.token), "token 原文不得落盘");
    assert!(
        raw.contains(&acp_common::token_sha256(&created.token)),
        "台账只存哈希"
    );
    let ledger = ledger_on_disk(&r.cfg);
    assert_eq!(
        ledger
            .get(&created.entry.share_id.to_string())
            .expect("entry")
            .activations,
        0
    );
}

#[test]
fn revoke_unknown_share_is_explicit_error() {
    let r = rig("unknown");
    assert!(matches!(
        r.service.revoke("00000000-0000-0000-0000-000000000000"),
        Err(RevokeError::Unknown(_))
    ));
}

#[test]
fn policy_save_failure_fails_closed_and_rolls_back_ledger() {
    let cfg = AgentConfig {
        data_dir: tmp_dir("rollback"),
        policy_path: Some(tmp_dir("rollback-policy")), // 目录路径：save 必败
        ..AgentConfig::default()
    };
    std::fs::create_dir_all(cfg.policy_path()).expect("policy dir");
    let audit = Arc::new(CaptureAudit::new());
    let policy = Arc::new(StdRwLock::new(PolicyTable::new()));
    let service = ShareService::open(
        &cfg,
        std::sync::Arc::new(crate::workspaces::WorkspaceStore::open_for_config(&cfg).unwrap()),
        policy.clone(),
        audit,
    )
    .expect("open");
    let created = service
        .create(spec(Scope::Sandbox, 3_600, 1), NOW)
        .expect("create");
    assert!(
        matches!(
            service.redeem("peerA", &created.token, NOW + 1),
            RedeemOutcome::Storage(_),
        ),
        "策略表写失败必须 fail-closed"
    );
    let ledger = ledger_on_disk(&cfg);
    let entry = ledger
        .get(&created.entry.share_id.to_string())
        .expect("entry");
    assert_eq!(entry.activations, 0, "回滚后激活次数必须归零");
    assert_eq!(entry.bound_peer, None, "回滚后不得绑定 peer");
}

#[test]
fn workspace_scope_requires_workspace_dir_owner_never_shareable() {
    let r = rig("scope");
    assert!(matches!(
        r.service.create(spec(Scope::Workspace, 60, 1), NOW),
        Err(ShareCreateError::WorkspaceUnconfigured),
    ));
    assert!(matches!(
        r.service.create(spec(Scope::Owner, 60, 1), NOW),
        Err(ShareCreateError::OwnerScope),
    ));
    let cfg = AgentConfig {
        data_dir: tmp_dir("scope-ok"),
        workspace_dir: Some(tmp_dir("scope-ws")),
        ..AgentConfig::default()
    };
    let audit = Arc::new(CaptureAudit::new());
    let policy = Arc::new(StdRwLock::new(PolicyTable::new()));
    let service = ShareService::open(
        &cfg,
        std::sync::Arc::new(crate::workspaces::WorkspaceStore::open_for_config(&cfg).unwrap()),
        policy,
        audit,
    )
    .expect("open");
    assert!(service.create(spec(Scope::Workspace, 60, 1), NOW).is_ok());
}
#[test]
fn workspace_share_targets_named_workspace_and_stamps_policy() {
    let cfg = AgentConfig {
        data_dir: tmp_dir("ws-named"),
        workspaces: vec![WorkspaceDef {
            id: "ws1".to_owned(),
            name: "p2p".to_owned(),
            dir: tmp_dir("ws-named-dir"),
        }],
        ..AgentConfig::default()
    };
    let audit = Arc::new(CaptureAudit::new());
    let policy = Arc::new(StdRwLock::new(PolicyTable::new()));
    let service = ShareService::open(
        &cfg,
        std::sync::Arc::new(crate::workspaces::WorkspaceStore::open_for_config(&cfg).unwrap()),
        policy.clone(),
        audit,
    )
    .expect("open");
    let ws_spec = ShareSpec {
        workspace: Some("ws1".to_owned()),
        ..spec(Scope::Workspace, 60, 1)
    };
    let created = service.create(ws_spec, NOW).expect("create");
    assert_eq!(created.entry.workspace.as_deref(), Some("ws1"));
    match service.redeem("peerW", &created.token, NOW + 1) {
        RedeemOutcome::Activated(grant) => {
            assert_eq!(
                grant.workspace.as_deref(),
                Some("ws1"),
                "策略条目必须携带工作区"
            );
        }
        other => panic!("兑换必须激活，实际 {other:?}"),
    }
}

#[test]
fn workspace_share_unknown_id_rejected_at_create() {
    let cfg = AgentConfig {
        data_dir: tmp_dir("ws-unknown"),
        workspaces: vec![WorkspaceDef {
            id: "ws1".to_owned(),
            name: "p2p".to_owned(),
            dir: tmp_dir("ws-unknown-dir"),
        }],
        ..AgentConfig::default()
    };
    let audit = Arc::new(CaptureAudit::new());
    let policy = Arc::new(StdRwLock::new(PolicyTable::new()));
    let service = ShareService::open(
        &cfg,
        std::sync::Arc::new(crate::workspaces::WorkspaceStore::open_for_config(&cfg).unwrap()),
        policy,
        audit,
    )
    .expect("open");
    let ws_spec = ShareSpec {
        workspace: Some("nope".to_owned()),
        ..spec(Scope::Workspace, 60, 1)
    };
    assert!(matches!(
        service.create(ws_spec, NOW),
        Err(ShareCreateError::WorkspaceUnknown(id)) if id == "nope",
    ));
}
