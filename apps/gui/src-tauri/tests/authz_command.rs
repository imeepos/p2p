//! gui-contract.md §18 authz 命令层回环（authz S3）：mock runtime 直调命令层
//! （ginvite_commands 装配口径），不起 webview 不起节点。覆盖：角色列表（内建
//! 四）/绑定 upsert/解绑/绑定列表/默认拒绝语义/默认角色读写/审计单写（禁双写）。

use std::path::{Path, PathBuf};

use p2p_authz::audit::audit_path;
use p2p_console::authz::{
    authz_bind, authz_bindings_list, authz_check, authz_default_role_get, authz_default_role_save,
    authz_role_list, authz_unbind,
};
use p2p_console::state::AppState;
use tauri::Manager;

/// 合法 base58 PeerId（解码恰 32 字节，与后端 parse_peer_id 同口径）。
fn peer(byte: u8) -> String {
    bs58::encode([byte; 32]).into_string()
}

fn cmd_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("authz-cmd-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

/// 退出清理：删不掉留告警不 panic（避免掩盖真失败原因）。
struct DirGuard(PathBuf);

impl Drop for DirGuard {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            eprintln!("[authz-cmd] 清理临时目录失败 {}: {e}", self.0.display());
        }
    }
}

fn app_for(tag: &str) -> (tauri::App<tauri::test::MockRuntime>, DirGuard) {
    let dir = cmd_dir(tag);
    let app = tauri::test::mock_app();
    app.handle().manage(AppState::new(dir.clone()));
    (app, DirGuard(dir))
}

fn audit_lines(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(audit_path(dir))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[tokio::test]
async fn role_list_returns_builtin_four() {
    let (app, _dir) = app_for("role-list");
    let state = app.handle().state::<AppState>();
    let report = authz_role_list(state).await.expect("读取角色列表");
    let ids: Vec<&str> = report.roles.iter().map(|r| r.role_id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["friend", "guest", "operator", "ally"],
        "内建四呈阶梯"
    );
    assert!(report.roles.iter().all(|r| r.builtin), "初次全为内建角色");
}

#[tokio::test]
async fn bind_upsert_unbind_roundtrip_with_single_audit_writes() {
    let (app, dir) = app_for("bind-unbind");
    let dir = dir.0.clone();
    let state = app.handle().state::<AppState>();
    let p = peer(1);

    let report = authz_bind(state.clone(), p.clone(), "operator".into(), None, None)
        .await
        .expect("首绑成功");
    assert!(report.created, "首绑 created=true");
    assert_eq!(report.role_id, "operator");

    // upsert 改绑：单值语义，created=false。
    let report = authz_bind(state.clone(), p.clone(), "ally".into(), Some(9_999), None)
        .await
        .expect("改绑成功");
    assert!(!report.created, "改绑 upsert created=false");
    assert_eq!(report.expires_at, Some(9_999));

    let bindings = authz_bindings_list(state.clone()).await.expect("绑定列表");
    assert_eq!(bindings.bindings.len(), 1, "peer 级单值");
    assert_eq!(bindings.bindings[0].role_id, "ally");
    assert_eq!(bindings.bindings[0].expires_at, Some(9_999));

    // 审计：两轮绑定恰好两条 bound（p2p-cli 逻辑层单写，禁双写）。
    let audit = audit_lines(&dir);
    assert_eq!(
        audit.iter().filter(|l| l.contains("authz.bound")).count(),
        2,
        "绑定恰落两条 bound 事件: {audit:?}"
    );

    let unbound = authz_unbind(state.clone(), p.clone())
        .await
        .expect("解绑成功");
    assert_eq!(unbound.role_id, "ally");
    assert!(
        authz_unbind(state.clone(), p.clone()).await.is_err(),
        "无绑定再解绑必须显式 Err"
    );
    let audit = audit_lines(&dir);
    assert_eq!(
        audit.iter().filter(|l| l.contains("authz.unbound")).count(),
        1,
        "解绑恰落一条 unbound 事件: {audit:?}"
    );
    assert!(
        authz_bindings_list(state)
            .await
            .expect("列表")
            .bindings
            .is_empty(),
        "解绑后绑定表为空"
    );
}

#[tokio::test]
async fn bind_unknown_role_and_invalid_peer_error_out() {
    let (app, _dir) = app_for("bind-err");
    let state = app.handle().state::<AppState>();
    let err = authz_bind(state.clone(), peer(2), "ghost".into(), None, None)
        .await
        .expect_err("未登记角色必须拒");
    assert!(err.contains("ghost"), "错误携带角色 id: {err}");
    let err = authz_bind(
        state.clone(),
        "not-base58".into(),
        "friend".into(),
        None,
        None,
    )
    .await
    .expect_err("非法 peer 必须拒");
    assert!(!err.is_empty(), "可读中文错误串");
}

/// 无角色绑定默认拒绝语义（§18.1 authz_check / 设计 §7 判定瀑布）。
#[tokio::test]
async fn check_reports_default_deny_and_permission_waterfall() {
    let (app, _dir) = app_for("check-deny");
    let state = app.handle().state::<AppState>();
    let p = peer(3);

    let report = authz_check(state.clone(), p.clone(), "llm.borrow".into())
        .await
        .expect("dry-run 判定");
    assert_eq!(report.decision, "deny");
    assert_eq!(report.reason, Some("NotBound"), "无绑定 = 默认拒绝");

    authz_bind(state.clone(), p.clone(), "guest".into(), None, None)
        .await
        .expect("绑 guest");
    let report = authz_check(state.clone(), p.clone(), "llm.borrow".into())
        .await
        .expect("dry-run 判定");
    assert_eq!(
        report.reason,
        Some("MissingPerm"),
        "guest 不含 llm.borrow（阶梯内降档拒绝）"
    );

    authz_bind(state.clone(), p.clone(), "ally".into(), None, None)
        .await
        .expect("升绑 ally");
    let report = authz_check(state.clone(), p.clone(), "llm.borrow".into())
        .await
        .expect("dry-run 判定");
    assert_eq!(report.decision, "allow");
    assert_eq!(report.reason, None, "allow 无 reason");

    let err = authz_check(state.clone(), p.clone(), "no.such".into())
        .await
        .expect_err("闭集外权限必须拒");
    assert!(err.contains("chat.send"), "错误提示附可用 key 清单: {err}");
}

#[tokio::test]
async fn default_role_get_save_roundtrip_and_validation() {
    let (app, dir) = app_for("default-role");
    let dir = dir.0.clone();
    let state = app.handle().state::<AppState>();

    let report = authz_default_role_get(state.clone()).await.expect("读取");
    assert_eq!(report.role_id, "friend", "缺省 friend（P1d）");

    let report = authz_default_role_save(state.clone(), "guest".into())
        .await
        .expect("设为 guest");
    assert_eq!(report.role_id, "guest");
    assert_eq!(
        authz_default_role_get(state.clone())
            .await
            .expect("读取")
            .role_id,
        "guest",
        "save 后 get 读到新值（持久化生效）"
    );
    let persisted = std::fs::read_to_string(dir.join("gui-config.json")).expect("配置文件");
    assert!(
        persisted.contains(r#""authzDefaultRole":"guest""#)
            || persisted.contains("\"authzDefaultRole\": \"guest\""),
        "落盘字段 camelCase: {persisted}"
    );

    let report = authz_default_role_save(state.clone(), String::new())
        .await
        .expect("空串 = 显式禁用");
    assert_eq!(report.role_id, "");

    let err = authz_default_role_save(state.clone(), "ghost".into())
        .await
        .expect_err("未登记角色必须拒");
    assert!(err.contains("未登记"), "错误语义为角色未登记: {err}");

    // 存储故障不静默吞成「未登记」：损坏 roles.json 后原样上浮存储错误。
    let authz_dir = dir.join("authz");
    std::fs::create_dir_all(&authz_dir).expect("建 authz 目录");
    std::fs::write(authz_dir.join("roles.json"), "{ not json").expect("写坏文件");
    let err = authz_default_role_save(state, "guest".into())
        .await
        .expect_err("存储故障必须显式报错");
    assert!(err.contains("损坏"), "存储错误原样上浮不静默: {err}");
}
