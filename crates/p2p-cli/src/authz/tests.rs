//! authz 逻辑层测试：临时目录全链驱动（角色生命周期 + 绑定判定链 +
//! 错误语义透传 + camelCase 报告形状）。

use std::path::PathBuf;

use super::access::{bind, check, unbind};
use super::roles::{role_create, role_delete, role_list, role_show};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2pcli-authz-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn peer(byte: u8) -> String {
    bs58::encode([byte; 32]).into_string()
}

fn perms(keys: &[&str]) -> Vec<String> {
    keys.iter().map(|k| (*k).to_owned()).collect()
}

#[test]
fn role_lifecycle_and_builtin_listing() {
    let dir = temp_dir("roles");
    let data = dir.to_str().unwrap();

    let listed = role_list(data).unwrap();
    assert_eq!(listed.roles.len(), 4);
    assert!(listed.roles.iter().all(|r| r.builtin));
    let friend = listed.roles.iter().find(|r| r.role_id == "friend").unwrap();
    assert_eq!(
        friend.permissions,
        vec!["chat.send", "chat.attachment", "a2a.discover"]
    );

    let created = role_create(
        data,
        "tester",
        Some("测试员"),
        &perms(&["acp.session", "chat.send"]),
        None,
    )
    .unwrap();
    assert_eq!(created.role.name, "测试员");
    assert_eq!(created.role.permissions.len(), 2, "重复 key 应去重");

    let shown = role_show(data, "tester").unwrap();
    assert_eq!(shown.role, created.role);

    assert!(role_create(data, "tester", None, &perms(&[]), None).is_err());
    assert!(role_delete(data, "friend").is_err(), "内建角色删除必须被拒");

    role_delete(data, "tester").unwrap();
    assert!(role_show(data, "tester").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bind_check_unbind_chain_reports_reasons() {
    let dir = temp_dir("chain");
    let data = dir.to_str().unwrap();
    let alice = peer(1);

    let bound = bind(data, &alice, "ally", Some(9_999_999_999), None).unwrap();
    assert!(bound.created);
    assert_eq!(bound.role_id, "ally");
    assert_eq!(bound.expires_at, Some(9_999_999_999));

    let allow = check(data, &alice, "llm.borrow").unwrap();
    assert_eq!(allow.decision, "allow");
    assert_eq!(allow.reason, None);

    // 重绑 friend（阶梯子集）：chat.send 仍 Allow，llm.borrow 落 MissingPerm。
    let rebound = bind(data, &alice, "friend", None, None).unwrap();
    assert!(!rebound.created, "重绑 = upsert");
    assert_eq!(check(data, &alice, "chat.send").unwrap().decision, "allow");
    let missing = check(data, &alice, "llm.borrow").unwrap();
    assert_eq!(
        (missing.decision, missing.reason),
        ("deny", Some("MissingPerm"))
    );

    let not_bound = check(data, &peer(2), "chat.send").unwrap();
    assert_eq!(not_bound.reason, Some("NotBound"));

    let unbound = unbind(data, &alice).unwrap();
    assert_eq!(unbound.role_id, "friend");
    assert_eq!(
        check(data, &alice, "chat.send").unwrap().reason,
        Some("NotBound")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn error_semantics_propagate_to_logic_layer() {
    let dir = temp_dir("errors");
    let data = dir.to_str().unwrap();
    let alice = peer(1);

    assert!(bind(data, &alice, "ghost", None, None).is_err());
    assert!(unbind(data, &alice).is_err(), "无绑定解绑必须报错");
    assert!(bind(data, "not-base58!", "friend", None, None).is_err());

    role_create(data, "tester", None, &perms(&["chat.send"]), None).unwrap();
    bind(data, &alice, "tester", None, None).unwrap();
    let err = role_delete(data, "tester").unwrap_err();
    assert!(err.contains("绑定"), "引用完整性错误必须可读: {err}");

    let err = check(data, &alice, "owner.superuser").unwrap_err();
    assert!(err.contains("闭集"), "表外 key 必须给出闭集提示: {err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reports_serialize_camel_case() {
    let dir = temp_dir("serde");
    let data = dir.to_str().unwrap();
    let alice = peer(1);
    bind(data, &alice, "friend", None, None).unwrap();
    let report = check(data, &alice, "chat.send").unwrap();
    let json = serde_json::to_value(&report).unwrap();
    assert!(json.get("peerId").is_some(), "camelCase: {json}");
    assert!(json.get("permission").is_some());
    assert_eq!(json["decision"], "allow");

    let denied = check(data, &alice, "llm.borrow").unwrap();
    let json = serde_json::to_value(&denied).unwrap();
    assert_eq!(json["decision"], "deny");
    assert_eq!(json["reason"], "MissingPerm");
    let _ = std::fs::remove_dir_all(&dir);
}
