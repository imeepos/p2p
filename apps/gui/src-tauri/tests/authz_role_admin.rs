//! gui-contract.md §18.5 角色管理命令面回环（authz 角色管理波）：mock runtime
//! 直调命令层（authz_command 装配口径）。覆盖：闭集枚举九 key、角色
//! create/update/delete 全链、内建拒改删、表外 key 拒、默认角色删除闸、审计。

use std::path::PathBuf;

use p2p_authz::audit::audit_path;
use p2p_console::authz::{
    authz_default_role_save, authz_permissions_list, authz_role_create, authz_role_delete,
    authz_role_list, authz_role_update,
};
use p2p_console::state::AppState;
use tauri::Manager;

fn cmd_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("authz-role-admin-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

/// 退出清理：删不掉留告警不 panic（避免掩盖真失败原因）。
struct DirGuard(PathBuf);

impl Drop for DirGuard {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            eprintln!(
                "[authz-role-admin] 清理临时目录失败 {}: {e}",
                self.0.display()
            );
        }
    }
}

fn app_for(tag: &str) -> (tauri::App<tauri::test::MockRuntime>, DirGuard) {
    let dir = cmd_dir(tag);
    let app = tauri::test::mock_app();
    app.handle().manage(AppState::new(dir.clone()));
    (app, DirGuard(dir))
}

fn audit_lines(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(audit_path(dir))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn perms(keys: &[&str]) -> Vec<String> {
    keys.iter().map(|k| (*k).to_owned()).collect()
}

#[tokio::test]
async fn permissions_list_returns_registry_order_nine_keys() {
    let report = authz_permissions_list().await.expect("闭集枚举");
    assert_eq!(
        report.permissions,
        perms(&[
            "chat.send",
            "chat.attachment",
            "a2a.discover",
            "a2a.invoke",
            "acp.session",
            "acp.execute",
            "llm.borrow",
            "repair.diag",
            "repair.fix",
        ]),
        "九 key 且 registry 顺序（§4 设计表逐字）"
    );
}

#[tokio::test]
async fn role_create_update_delete_full_roundtrip() {
    let (app, dir) = app_for("roundtrip");
    let dir = dir.0.clone();
    let state = app.handle().state::<AppState>();

    let report = authz_role_create(
        state.clone(),
        "tester".into(),
        "测试员".into(),
        perms(&["acp.session", "chat.send", "acp.session"]),
        "备注".into(),
    )
    .await
    .expect("创建成功");
    assert_eq!(report.role.role_id, "tester");
    assert!(!report.role.builtin);
    assert_eq!(
        report.role.permissions,
        perms(&["acp.session", "chat.send"]),
        "权限保序去重"
    );

    let report = authz_role_update(
        state.clone(),
        "tester".into(),
        "新名".into(),
        perms(&["repair.diag"]),
        "新备注".into(),
    )
    .await
    .expect("修改成功");
    assert_eq!(report.role.role_id, "tester", "role_id 不可变");
    assert_eq!(report.role.name, "新名");
    assert_eq!(report.role.permissions, perms(&["repair.diag"]));

    let ids: Vec<String> = authz_role_list(state.clone())
        .await
        .expect("列表")
        .roles
        .into_iter()
        .map(|r| r.role_id)
        .collect();
    assert!(ids.contains(&"tester".into()), "改后仍在列表: {ids:?}");

    let report = authz_role_delete(state.clone(), "tester".into())
        .await
        .expect("删除成功");
    assert_eq!(report.role_id, "tester");
    let ids: Vec<String> = authz_role_list(state.clone())
        .await
        .expect("列表")
        .roles
        .into_iter()
        .map(|r| r.role_id)
        .collect();
    assert!(!ids.contains(&"tester".into()), "删后不在列表: {ids:?}");

    let audit = audit_lines(&dir);
    assert_eq!(
        audit
            .iter()
            .filter(|l| l.contains("authz.role.created"))
            .count(),
        1,
        "恰一条 role.created: {audit:?}"
    );
    assert_eq!(
        audit
            .iter()
            .filter(|l| l.contains("authz.role.deleted"))
            .count(),
        1,
        "恰一条 role.deleted: {audit:?}"
    );
}

#[tokio::test]
async fn role_create_rejects_builtin_conflict_and_outside_perm() {
    let (app, _dir) = app_for("create-err");
    let state = app.handle().state::<AppState>();
    let err = authz_role_create(
        state.clone(),
        "friend".into(),
        "冒充内建".into(),
        perms(&["chat.send"]),
        String::new(),
    )
    .await
    .expect_err("内建同 id 必须拒");
    assert!(err.contains("friend"), "错误携带角色 id: {err}");
    let err = authz_role_create(
        state.clone(),
        "tester".into(),
        "x".into(),
        perms(&["owner.superuser"]),
        String::new(),
    )
    .await
    .expect_err("表外 key 必须拒");
    assert!(err.contains("owner.superuser"), "错误携带表外 key: {err}");
}

#[tokio::test]
async fn role_update_rejects_builtin_and_missing() {
    let (app, _dir) = app_for("update-err");
    let state = app.handle().state::<AppState>();
    let err = authz_role_update(
        state.clone(),
        "friend".into(),
        "x".into(),
        perms(&["chat.send"]),
        String::new(),
    )
    .await
    .expect_err("内建拒改");
    assert!(err.contains("内建"), "可读中文错误: {err}");
    let err = authz_role_update(
        state,
        "ghost".into(),
        "x".into(),
        perms(&["chat.send"]),
        String::new(),
    )
    .await
    .expect_err("未登记拒改");
    assert!(err.contains("不存在"), "可读中文错误: {err}");
}

#[tokio::test]
async fn role_delete_blocked_while_default_role() {
    let (app, _dir) = app_for("default-gate");
    let state = app.handle().state::<AppState>();
    authz_role_create(
        state.clone(),
        "tester".into(),
        "x".into(),
        perms(&["chat.send"]),
        String::new(),
    )
    .await
    .expect("前置创建");
    authz_default_role_save(state.clone(), "tester".into())
        .await
        .expect("设为默认角色");

    let err = authz_role_delete(state.clone(), "tester".into())
        .await
        .expect_err("默认角色必须拒删");
    assert!(err.contains("默认角色"), "闸错误可读中文: {err}");
    assert!(
        authz_role_list(state.clone())
            .await
            .expect("列表")
            .roles
            .iter()
            .any(|r| r.role_id == "tester"),
        "被拒后角色仍在"
    );

    authz_default_role_save(state.clone(), "guest".into())
        .await
        .expect("换默认角色");
    authz_role_delete(state, "tester".into())
        .await
        .expect("换闸后可删");
}
