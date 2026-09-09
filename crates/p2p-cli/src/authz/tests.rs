//! authz 逻辑层测试：临时目录全链驱动（角色生命周期 + 绑定判定链 +
//! 错误语义透传 + camelCase 报告形状）。

use std::path::{Path, PathBuf};

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

/// P1b 接线验收（authz-a3-plan §1 S2）：role create/delete、bind/unbind 四类
/// 管理面事件按序落 audit.jsonl；check（dry-run）与失败操作不落账。
#[test]
fn management_ops_write_audit_events_in_order() {
    let dir = temp_dir("audit");
    let data = dir.to_str().unwrap();
    let alice = peer(1);

    role_create(data, "tester", None, &perms(&["chat.send"]), None).unwrap();
    bind(data, &alice, "tester", None, None).unwrap();
    bind(data, &alice, "friend", None, None).unwrap(); // upsert
    unbind(data, &alice).unwrap();
    role_delete(data, "tester").unwrap();

    let path = p2p_authz::audit::audit_path(Path::new(data));
    let text = std::fs::read_to_string(path).unwrap();
    let kinds: Vec<String> = text
        .lines()
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(l).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            "authz.role.created",
            "authz.bound",
            "authz.bound",
            "authz.unbound",
            "authz.role.deleted",
        ],
        "管理面事件按操作序落账: {text}"
    );
    let bound_line = text.lines().nth(1).unwrap();
    assert!(
        bound_line.contains("\"peerId\""),
        "绑定快照 camelCase: {bound_line}"
    );
    let rebound = serde_json::from_str::<serde_json::Value>(text.lines().nth(2).unwrap()).unwrap();
    assert_eq!(rebound["note"], "upsert", "重绑以 note 区分");
    assert!(bound_line.contains("\"roleId\":\"tester\""));

    // check 是 dry-run：不产生新事件。
    let before = text.lines().count();
    check(data, &alice, "chat.send").unwrap();
    let after = std::fs::read_to_string(p2p_authz::audit::audit_path(Path::new(data))).unwrap();
    assert_eq!(after.lines().count(), before, "dry-run 不落账");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 失败的管理面操作不落账（只有真实变更才产生事件）。
#[test]
fn failed_ops_write_no_audit_events() {
    let dir = temp_dir("audit-fail");
    let data = dir.to_str().unwrap();
    let alice = peer(1);

    assert!(bind(data, &alice, "ghost", None, None).is_err());
    assert!(unbind(data, &alice).is_err());
    assert!(role_delete(data, "friend").is_err());

    let path = p2p_authz::audit::audit_path(Path::new(data));
    assert!(!path.exists(), "失败操作不得产生审计文件或事件");
    let _ = std::fs::remove_dir_all(&dir);
}
