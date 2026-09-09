//! share 兑换的 authz 绑定侧测试（A2 回归修复）：激活即自动绑 guest（准入
//! 双查的绑定侧）、已有绑定不覆盖（更严者为准）、revoke 级联只回收自动绑定。

use std::path::Path;

use acp_common::Scope;
use p2p_authz::{store as authz_store, Authz as AuthzStore, SystemClock};

use crate::config::AgentConfig;

use super::testutil::{rig, spec, NOW};
use super::{RedeemOutcome, SHARE_AUTO_BIND_NOTE};

fn bindings(cfg: &AgentConfig) -> Vec<(String, String, String)> {
    authz_store::load_bindings(Path::new(&cfg.data_dir))
        .expect("authz bindings readable")
        .into_iter()
        .map(|b| (b.peer_id, b.role_id, b.note))
        .collect()
}

#[test]
fn redeem_activation_auto_binds_guest() {
    let r = rig("bind-auto");
    let created = r
        .service
        .create(spec(Scope::Sandbox, 3_600, 1), NOW)
        .expect("create");
    assert!(matches!(
        r.service.redeem("peerA", &created.token, NOW + 1),
        RedeemOutcome::Activated(_)
    ));
    let got = bindings(&r.cfg);
    assert_eq!(
        got,
        vec![(
            "peerA".to_owned(),
            "guest".to_owned(),
            SHARE_AUTO_BIND_NOTE.to_owned()
        )],
        "激活必须自动绑定 guest（准入双查的绑定侧）"
    );
}

#[test]
fn redeem_never_overwrites_existing_binding() {
    let r = rig("bind-keep");
    AuthzStore::new(Path::new(&r.cfg.data_dir), SystemClock)
        .bind("peerA", "operator", None, "manual")
        .expect("manual pre-bind");
    let created = r
        .service
        .create(spec(Scope::Sandbox, 3_600, 1), NOW)
        .expect("create");
    assert!(matches!(
        r.service.redeem("peerA", &created.token, NOW + 1),
        RedeemOutcome::Activated(_)
    ));
    let got = bindings(&r.cfg);
    assert_eq!(got.len(), 1, "不得新建第二条绑定");
    assert_eq!(
        (got[0].1.as_str(), got[0].2.as_str()),
        ("operator", "manual"),
        "人工改绑必须保留（更严者为准）"
    );
}

#[test]
fn revoke_cascades_auto_binding_but_keeps_manual() {
    let r = rig("bind-cascade");
    AuthzStore::new(Path::new(&r.cfg.data_dir), SystemClock)
        .bind("peerManual", "operator", None, "manual")
        .expect("manual pre-bind");
    let auto = r
        .service
        .create(spec(Scope::Sandbox, 3_600, 1), NOW)
        .expect("create");
    assert!(matches!(
        r.service.redeem("peerAuto", &auto.token, NOW + 1),
        RedeemOutcome::Activated(_)
    ));
    r.service
        .revoke(&auto.entry.share_id.to_string())
        .expect("revoke");
    let got = bindings(&r.cfg);
    assert_eq!(got.len(), 1, "revoke 必须级联移除自动绑定");
    assert_eq!(
        (got[0].0.as_str(), got[0].1.as_str(), got[0].2.as_str()),
        ("peerManual", "operator", "manual"),
        "人工绑定必须原样保留"
    );
}
