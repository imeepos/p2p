//! ShareService 兑换路径测试（设计 §4）：五路径 + 级联撤销边界 + 审计留痕。

use std::sync::{Arc, RwLock as StdRwLock};

use acp_common::policy::PolicyTable;
use acp_common::{AskRoute, Scope, ShareEntry, SHARE_FINGERPRINT_PREFIX};

use super::testutil::{hand_ledger, ledger_on_disk, rig, spec, tmp_dir, NOW};
use super::{RedeemOutcome, ShareDenyKind, ShareService};
use crate::audit::{AuditEvent, CaptureAudit};
use crate::config::AgentConfig;

#[test]
fn redeem_success_activates_writes_policy_and_audits() {
    let r = rig("success");
    let created = r
        .service
        .create(spec(Scope::Sandbox, 3_600, 1), NOW)
        .expect("create");
    let share_id = created.entry.share_id.to_string();
    match r.service.redeem("peerA", &created.token, NOW + 10) {
        RedeemOutcome::Activated(grant) => {
            assert_eq!(grant.scope, Scope::Sandbox);
            assert_eq!(
                grant.fingerprint,
                format!("{SHARE_FINGERPRINT_PREFIX}{share_id}")
            );
        }
        other => panic!("期望激活，实得 {other:?}"),
    }
    let table = r.policy.read().unwrap_or_else(|p| p.into_inner());
    let grant = table.lookup("peerA").expect("策略表应有 peerA");
    assert_eq!(
        grant.fingerprint,
        format!("{SHARE_FINGERPRINT_PREFIX}{share_id}")
    );
    drop(table);
    let ledger = ledger_on_disk(&r.cfg);
    assert_eq!(
        ledger.get(&share_id).expect("entry").activations,
        1,
        "激活次数必须持久化"
    );
    assert!(
        r.audit
            .contains(|e| matches!(e, AuditEvent::ShareRedeemed { peer, share_id: hit } if peer == "peerA" && hit == &share_id)),
        "应审计 share-redeemed"
    );
}

#[test]
fn redeem_five_paths_full_matrix() {
    // 成功
    let r = rig("five");
    let ok = r
        .service
        .create(spec(Scope::Sandbox, 3_600, 1), NOW)
        .expect("create");
    assert!(matches!(
        r.service.redeem("peerA", &ok.token, NOW + 1),
        RedeemOutcome::Activated(_)
    ));
    // 他人重用：token 匹配但绑定 peerA，peerB 拒绝
    assert!(matches!(
        r.service.redeem("peerB", &ok.token, NOW + 2),
        RedeemOutcome::Denied {
            kind: ShareDenyKind::Reuse,
            ..
        }
    ));
    // 超次：绑定者本人再兑换且次数已尽
    assert!(matches!(
        r.service.redeem("peerA", &ok.token, NOW + 3),
        RedeemOutcome::Denied {
            kind: ShareDenyKind::Exhausted,
            ..
        }
    ));
    // 撤销
    let rv = r
        .service
        .create(spec(Scope::Sandbox, 3_600, 1), NOW)
        .expect("create");
    r.service
        .revoke(&rv.entry.share_id.to_string())
        .expect("revoke");
    assert!(matches!(
        r.service.redeem("peerC", &rv.token, NOW + 4),
        RedeemOutcome::Denied {
            kind: ShareDenyKind::Revoked,
            ..
        }
    ));
    // 过期
    let ex = r
        .service
        .create(spec(Scope::Sandbox, 10, 1), NOW)
        .expect("create");
    assert!(matches!(
        r.service.redeem("peerD", &ex.token, NOW + 11),
        RedeemOutcome::Denied {
            kind: ShareDenyKind::Expired,
            ..
        }
    ));
    // 审计面：五条事件键全部留痕
    for key in [
        "share-redeemed",
        "share-reuse-denied",
        "share-exhausted",
        "share-revoked",
        "share-expired",
    ] {
        assert!(
            r.audit.contains(|e| e.kind() == key),
            "缺审计事件 {key}: {:?}",
            r.audit
                .snapshot()
                .iter()
                .map(|e| e.kind())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn redeem_unknown_token_not_matched_without_share_audit() {
    let r = rig("nomatch");
    assert!(matches!(
        r.service.redeem("peerA", "deadbeef", NOW),
        RedeemOutcome::NotMatched
    ));
    assert!(
        !r.audit.contains(|e| e.kind().starts_with("share-")),
        "不匹配不留 share 审计"
    );
}

#[test]
fn revoke_cascades_only_share_sourced_policy_entry() {
    let cfg = AgentConfig {
        data_dir: tmp_dir("cascade"),
        ..AgentConfig::default()
    };
    let audit = Arc::new(CaptureAudit::new());
    let policy = Arc::new(StdRwLock::new(PolicyTable::new()));
    // 手工条目必须先落盘再 open（service 持内存账，open 后不再重读文件）。
    {
        let mut table = policy.write().unwrap_or_else(|p| p.into_inner());
        table.grant(
            "peerB",
            acp_common::PeerPolicy {
                scope: Scope::Sandbox,
                allow_mcp: Vec::new(),
                ask_route: AskRoute::RemoteGui,
                note: String::new(),
                granted_at: "2026-01-01T00:00:00Z".to_owned(),
                fingerprint: "manual".to_owned(),
            },
        );
        table.save(&cfg.policy_path()).expect("policy");
    }
    let mut entry = ShareEntry::new(
        spec(Scope::Sandbox, 3_600, 1),
        "tok",
        NOW,
        uuid::Uuid::new_v4(),
    );
    let share_id = entry.share_id.to_string();
    entry.bound_peer = Some("peerB".to_owned());
    hand_ledger(&cfg, entry);
    let service = ShareService::open(&cfg, policy.clone(), audit).expect("open");

    let report = service.revoke(&share_id).expect("revoke");
    assert!(!report.policy_removed, "非 share 来源条目不得级联删除");
    let table = policy.read().unwrap_or_else(|p| p.into_inner());
    assert!(table.lookup("peerB").is_some(), "手工条目应保留");
    drop(table);

    // share 来源条目：创建 → 兑换 → 撤销级联删除。
    let created = service
        .create(spec(Scope::Sandbox, 3_600, 1), NOW)
        .expect("create");
    service.redeem("peerA", &created.token, NOW + 1);
    let report = service
        .revoke(&created.entry.share_id.to_string())
        .expect("revoke");
    assert!(report.policy_removed);
    let table = policy.read().unwrap_or_else(|p| p.into_inner());
    assert!(table.lookup("peerA").is_none(), "share 来源条目应级联删除");
}
