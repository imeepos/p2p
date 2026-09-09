//! 管理面操作测试（ops.rs / ops_binding.rs，临时目录 + 脚本化时钟全链驱动）。
//! 覆盖验收第 4 项的 ops 级 Allow/Deny 分支与第 6 项引用完整性（有绑定删角色即拒）。

use std::path::PathBuf;

use crate::clock::{Clock, SystemClock};
use crate::decision::{Decision, DenyReason};
use crate::errors::AuthzError;
use crate::ops::Authz;
use crate::ops_binding::BoundBinding;
use crate::permissions::Permission;
use crate::role::{builtin_roles, Role};

struct FakeClock(std::cell::Cell<u64>);

impl Clock for FakeClock {
    fn now_unix(&self) -> u64 {
        self.0.get()
    }
}

impl FakeClock {
    fn at(secs: u64) -> Self {
        FakeClock(std::cell::Cell::new(secs))
    }
    fn advance(&self, secs: u64) {
        self.0.set(self.0.get() + secs);
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-authz-ops-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn perms(keys: &[&str]) -> Vec<String> {
    keys.iter().map(|k| (*k).to_owned()).collect()
}

fn expect_err<T>(result: Result<T, AuthzError>, what: &str) -> AuthzError {
    result.err().unwrap_or_else(|| panic!("{what}: 应失败却成功"))
}

#[test]
fn create_list_show_delete_custom_role_lifecycle() {
    let dir = temp_dir("lifecycle");
    let authz = Authz::new(&dir, SystemClock);
    assert_eq!(authz.list_roles().unwrap(), builtin_roles());

    let created = authz
        .create_role("tester", "测试员", &perms(&["acp.session", "chat.send", "acp.session"]), "备注")
        .unwrap();
    assert_eq!(created.role_id, "tester");
    assert_eq!(created.permissions, vec![Permission::ACP_SESSION, Permission::CHAT_SEND]);
    assert!(!created.builtin);

    let listed = authz.list_roles().unwrap();
    assert_eq!(listed.len(), builtin_roles().len() + 1);
    let shown = authz.show_role("tester").unwrap();
    assert_eq!(shown, created);
    assert_eq!(authz.show_role("friend").unwrap().role_id, "friend");

    assert_eq!(authz.delete_role("tester").unwrap().role_id, "tester");
    assert_eq!(authz.list_roles().unwrap(), builtin_roles());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_role_rejections_are_explicit() {
    let dir = temp_dir("reject");
    let authz = Authz::new(&dir, SystemClock);
    assert!(matches!(
        expect_err(
            authz.create_role("Bad_Id", "x", &perms(&["chat.send"]), ""),
            "非法 id"
        ),
        AuthzError::InvalidRoleId(_)
    ));
    assert!(matches!(
        expect_err(
            authz.create_role("friend", "冒充内建", &perms(&["chat.send"]), ""),
            "内建同 id"
        ),
        AuthzError::BuiltinImmutable(_)
    ));
    assert!(matches!(
        expect_err(
            authz.create_role("tester", "x", &perms(&["owner.superuser"]), ""),
            "表外权限"
        ),
        AuthzError::UnknownPermission(_)
    ));
    authz.create_role("tester", "x", &perms(&["chat.send"]), "").unwrap();
    assert!(matches!(
        expect_err(
            authz.create_role("tester", "y", &perms(&["chat.send"]), ""),
            "重复创建"
        ),
        AuthzError::RoleExists(_)
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_role_rejects_builtin_missing_and_referenced() {
    let dir = temp_dir("delete");
    let clock = FakeClock::at(1_000);
    let authz = Authz::new(&dir, clock);
    authz.create_role("tester", "x", &perms(&["chat.send"]), "").unwrap();

    assert!(matches!(
        expect_err(authz.delete_role("friend"), "删内建"),
        AuthzError::BuiltinImmutable(_)
    ));
    assert!(matches!(
        expect_err(authz.delete_role("ghost"), "删不存在"),
        AuthzError::RoleNotFound(_)
    ));

    authz.bind("peer-a", "tester", None, "").unwrap();
    let referenced = expect_err(authz.delete_role("tester"), "有引用删除");
    assert!(
        matches!(referenced, AuthzError::RoleReferenced(_)),
        "有绑定引用时删除必须被拒（验收第 6 项），实得: {referenced}"
    );
    assert!(authz.show_role("tester").is_ok(), "被拒后角色仍在");

    authz.unbind("peer-a").unwrap();
    assert_eq!(authz.delete_role("tester").unwrap().role_id, "tester");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bind_unbind_check_full_chain_with_injected_clock() {
    let dir = temp_dir("chain");
    let clock = FakeClock::at(1_000);
    let authz = Authz::new(&dir, &clock);

    let bound = authz.bind("peer-a", "friend", Some(2_000), "临时访客").unwrap();
    assert!(matches!(bound, BoundBinding { created: true, .. }));
    assert_eq!(bound.binding.granted_at, 1_000, "granted_at 由 Clock 注入");

    assert_eq!(
        authz.check("peer-a", Permission::CHAT_SEND).unwrap(),
        Decision::Allow
    );
    clock.advance(1_000);
    assert_eq!(
        authz.check("peer-a", Permission::CHAT_SEND).unwrap(),
        Decision::Deny(DenyReason::Expired)
    );

    let rebound = authz.bind("peer-a", "ally", None, "").unwrap();
    assert!(!rebound.created, "重绑 = upsert");
    assert_eq!(authz.check("peer-a", Permission::REPAIR_FIX).unwrap(), Decision::Allow);

    assert!(matches!(
        expect_err(authz.bind("peer-b", "ghost", None, ""), "绑不存在角色"),
        AuthzError::RoleNotFound(_)
    ));
    assert!(matches!(
        expect_err(authz.unbind("peer-b"), "解绑无条目"),
        AuthzError::NotBound(_)
    ));

    let removed = authz.unbind("peer-a").unwrap();
    assert_eq!(removed.role_id, "ally");
    assert_eq!(
        authz.check("peer-a", Permission::REPAIR_FIX).unwrap(),
        Decision::Deny(DenyReason::NotBound)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn persisted_state_survives_facade_rebuild() {
    let dir = temp_dir("persist");
    let authz = Authz::new(&dir, FakeClock::at(1_000));
    authz.create_role("tester", "x", &perms(&["acp.execute"]), "").unwrap();
    authz.bind("peer-a", "tester", None, "").unwrap();

    let reopened = Authz::new(&dir, SystemClock);
    assert_eq!(
        reopened.check("peer-a", Permission::ACP_EXECUTE).unwrap(),
        Decision::Allow
    );
    let binding = crate::store::load_bindings(reopened.data_dir())
        .unwrap()
        .into_iter()
        .find(|b| b.peer_id == "peer-a")
        .unwrap();
    assert_eq!(binding.role_id, "tester");
    assert_eq!(binding.granted_at, 1_000, "重开 facade 后 granted_at 仍在");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn role_model_serialization_roundtrip() {
    // Role 序列化往返：存储层形状回归（name/note/builtin 完整保留）。
    let role = Role {
        role_id: "t".to_owned(),
        name: "名".to_owned(),
        permissions: vec![Permission::CHAT_SEND],
        builtin: false,
        note: "n".to_owned(),
    };
    let parsed: Role = serde_json::from_str(&serde_json::to_string(&role).unwrap()).unwrap();
    assert_eq!(parsed, role);
}
