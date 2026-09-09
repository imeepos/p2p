//! 判定瀑布与引擎变更语义测试（engine.rs 的测试面，经公开 API 驱动）。
//! 覆盖：NotBound / Expired（含到期秒边界）/ BrokenRole / MissingPerm / Allow
//! 五分支各至少一测；内建优先解析；插入冲突显式报错；单值重绑 upsert。

use crate::binding::Binding;
use crate::decision::{Decision, DenyReason};
use crate::engine::AuthzEngine;
use crate::errors::AuthzError;
use crate::permissions::Permission;
use crate::role::Role;

fn peer(id: &str) -> String {
    format!("peer-{id}")
}

fn friend_bound(peer_id: &str, expires_at: Option<u64>) -> AuthzEngine {
    AuthzEngine::from_parts(
        vec![],
        vec![Binding {
            peer_id: peer_id.to_owned(),
            role_id: "friend".to_owned(),
            granted_at: 1_000,
            note: String::new(),
            expires_at,
        }],
    )
}

fn custom_role(id: &str, builtin: bool, perm: Permission) -> Role {
    Role {
        role_id: id.to_owned(),
        name: id.to_owned(),
        permissions: vec![perm],
        builtin,
        note: String::new(),
    }
}

#[test]
fn unbound_peer_is_denied_not_bound() {
    let engine = friend_bound(&peer("a"), None);
    assert_eq!(
        engine.check(&peer("b"), Permission::CHAT_SEND, 1_000),
        Decision::Deny(DenyReason::NotBound)
    );
}

#[test]
fn expired_binding_is_denied_expired() {
    let engine = friend_bound(&peer("a"), Some(2_000));
    assert_eq!(
        engine.check(&peer("a"), Permission::CHAT_SEND, 1_999),
        Decision::Allow,
        "到期前一秒仍 Allow（含边界语义见 binding.rs）"
    );
    assert_eq!(
        engine.check(&peer("a"), Permission::CHAT_SEND, 2_000),
        Decision::Deny(DenyReason::Expired)
    );
}

#[test]
fn dangling_role_reference_is_denied_broken_role() {
    let engine = AuthzEngine::from_parts(
        vec![],
        vec![Binding {
            peer_id: peer("a"),
            role_id: "ghost".to_owned(),
            granted_at: 1_000,
            note: String::new(),
            expires_at: None,
        }],
    );
    assert_eq!(
        engine.check(&peer("a"), Permission::CHAT_SEND, 1_000),
        Decision::Deny(DenyReason::BrokenRole)
    );
}

#[test]
fn missing_permission_is_denied_missing_perm() {
    let engine = friend_bound(&peer("a"), None);
    assert_eq!(
        engine.check(&peer("a"), Permission::LLM_BORROW, 1_000),
        Decision::Deny(DenyReason::MissingPerm)
    );
}

#[test]
fn allow_path_and_unbind_returns_to_not_bound() {
    let mut engine = friend_bound(&peer("a"), None);
    assert_eq!(
        engine.check(&peer("a"), Permission::CHAT_SEND, 1_000),
        Decision::Allow
    );
    assert!(engine.remove_binding(&peer("a")).is_some());
    assert_eq!(
        engine.check(&peer("a"), Permission::CHAT_SEND, 1_000),
        Decision::Deny(DenyReason::NotBound)
    );
}

#[test]
fn builtin_and_custom_roles_resolve() {
    let mut engine = AuthzEngine::new();
    assert!(engine.lookup_role("ally").is_some(), "内建角色代码内可达");
    engine
        .insert_custom_role(custom_role("tester", false, Permission::ACP_SESSION))
        .unwrap();
    assert!(engine.lookup_role("tester").is_some());
    assert_eq!(
        engine.check(&peer("a"), Permission::ACP_SESSION, 1_000),
        Decision::Deny(DenyReason::NotBound)
    );
}

#[test]
fn insert_conflicts_are_explicit() {
    let mut engine = AuthzEngine::new();
    engine
        .insert_custom_role(custom_role("tester", false, Permission::CHAT_SEND))
        .unwrap();
    assert!(matches!(
        engine.insert_custom_role(custom_role("tester", false, Permission::CHAT_SEND)),
        Err(AuthzError::RoleExists(_))
    ));
    assert!(matches!(
        engine.insert_custom_role(custom_role("friend", false, Permission::CHAT_SEND)),
        Err(AuthzError::BuiltinImmutable(_))
    ));
    assert!(matches!(
        engine.insert_custom_role(custom_role("ghost", true, Permission::CHAT_SEND)),
        Err(AuthzError::BuiltinImmutable(_))
    ));
}

#[test]
fn from_parts_drops_builtin_id_collision_defensively() {
    let colliding = custom_role("friend", false, Permission::REPAIR_FIX);
    let engine = AuthzEngine::from_parts(vec![colliding], vec![]);
    assert!(
        !engine
            .lookup_role("friend")
            .unwrap()
            .has(Permission::REPAIR_FIX),
        "同 id 自定义不得改写内建权限集"
    );
}

#[test]
fn upsert_rebind_is_single_value_and_role_binding_count_works() {
    let mut engine = AuthzEngine::new();
    let bind = |engine: &mut AuthzEngine, role_id: &str| {
        engine.upsert_binding(Binding {
            peer_id: peer("a"),
            role_id: role_id.to_owned(),
            granted_at: 1_000,
            note: String::new(),
            expires_at: None,
        })
    };
    assert!(bind(&mut engine, "friend"));
    assert!(!bind(&mut engine, "ally"), "重绑 = upsert 非新建");
    assert_eq!(engine.binding(&peer("a")).unwrap().role_id, "ally");
    assert_eq!(engine.role_binding_count("ally"), 1);
    assert_eq!(engine.role_binding_count("guest"), 0);
}

#[test]
fn remove_custom_role_rejects_builtin_and_missing() {
    let mut engine = AuthzEngine::new();
    assert!(matches!(
        engine.remove_custom_role("friend"),
        Err(AuthzError::BuiltinImmutable(_))
    ));
    assert!(matches!(
        engine.remove_custom_role("tester"),
        Err(AuthzError::RoleNotFound(_))
    ));
    engine
        .insert_custom_role(custom_role("tester", false, Permission::CHAT_SEND))
        .unwrap();
    assert_eq!(
        engine.remove_custom_role("tester").unwrap().role_id,
        "tester"
    );
}
