//! import friends 回填测试：首跑绑定 N、重跑零新增（幂等）、过期绑定不改写、
//! 空配置禁用、缺角色整体跳过；审计恰为 N 条 authz.bound + 每跑一条 import 摘要。

use std::path::{Path, PathBuf};

use super::import_friends;
use crate::{Authz, SystemClock};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-authz-importf-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn audit_text(dir: &Path) -> String {
    std::fs::read_to_string(crate::audit::audit_path(dir)).unwrap_or_default()
}

fn peers() -> Vec<String> {
    vec!["peer-a".into(), "peer-b".into(), "peer-c".into()]
}

/// 首跑绑定 3、重跑 0 新增（幂等）：绑定表字节不变，审计仅多一条摘要。
#[test]
fn first_run_binds_and_rerun_is_noop() {
    let dir = temp_dir("idempotent");
    let first = import_friends(&dir, &peers(), "friend").unwrap();
    assert_eq!(
        first,
        super::ImportFriendsReport {
            scanned: 3,
            bound: 3,
            skipped: 0,
            disabled: false
        }
    );
    let bindings_path = dir.join("authz/bindings.json");
    let after_first = std::fs::read(&bindings_path).unwrap();

    let second = import_friends(&dir, &peers(), "friend").unwrap();
    assert_eq!(second.bound, 0, "重跑零新增");
    assert_eq!(second.skipped, 3);
    assert_eq!(
        std::fs::read(&bindings_path).unwrap(),
        after_first,
        "重跑不改写绑定表"
    );
    let text = audit_text(&dir);
    assert_eq!(
        text.matches("authz.bound").count(),
        3,
        "仅首跑落 3 条绑定事件: {text}"
    );
    assert_eq!(
        text.matches("authz.import.friends").count(),
        2,
        "每跑恰一条摘要"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 摘要事件携带 bound/skipped 结构化计数（A-4）。
#[test]
fn summary_audit_carries_counts() {
    let dir = temp_dir("summary");
    import_friends(&dir, &peers(), "friend").unwrap();
    let text = audit_text(&dir);
    let line = text
        .lines()
        .find(|l| l.contains("authz.import.friends"))
        .expect("摘要事件存在");
    assert!(line.contains(r#""bound":3"#), "bound 计数: {line}");
    assert!(line.contains(r#""skipped":0"#), "skipped 计数: {line}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 已过期绑定跳过（过期是判定语义，回填不改写用户显式绑定）。
#[test]
fn expired_binding_is_left_untouched() {
    let dir = temp_dir("expired");
    Authz::new(&dir, SystemClock)
        .bind("peer-a", "ally", Some(1), "owner-set")
        .unwrap();
    let report = import_friends(&dir, &peers(), "friend").unwrap();
    assert_eq!(report.bound, 2);
    assert_eq!(report.skipped, 1);
    let binding = Authz::new(&dir, SystemClock)
        .load_engine()
        .unwrap()
        .binding("peer-a")
        .unwrap()
        .clone();
    assert_eq!(binding.role_id, "ally", "角色不改写");
    assert_eq!(binding.expires_at, Some(1), "过期时间不改写");
    assert_eq!(binding.note, "owner-set");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 空串配置 = 禁用：零存储零审计。
#[test]
fn empty_role_disables_everything() {
    let dir = temp_dir("disabled");
    let report = import_friends(&dir, &peers(), "").unwrap();
    assert!(report.disabled);
    assert_eq!(report.bound, 0);
    assert!(!dir.join("authz/bindings.json").exists(), "禁用不写存储");
    assert!(!crate::audit::audit_path(&dir).exists(), "禁用不落审计");
    let _ = std::fs::remove_dir_all(&dir);
}

/// default_role 角色不存在：整体跳过（不部分绑定），执行仍落摘要审计。
#[test]
fn missing_default_role_skips_all_and_audits() {
    let dir = temp_dir("norole");
    let report = import_friends(&dir, &peers(), "ghost").unwrap();
    assert_eq!(report.bound, 0);
    assert_eq!(report.skipped, 3);
    assert!(!dir.join("authz/bindings.json").exists(), "缺角色不写绑定");
    assert!(
        audit_text(&dir).contains("authz.import.friends"),
        "执行事实落审计"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 损坏绑定表 → 显式报错上抛（红线 2），不静默半跑。
#[test]
fn corrupted_store_fails_explicitly() {
    let dir = temp_dir("corrupt");
    let authz_dir = dir.join("authz");
    std::fs::create_dir_all(&authz_dir).unwrap();
    std::fs::write(authz_dir.join("bindings.json"), "{ not json").unwrap();
    assert!(import_friends(&dir, &peers(), "friend").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
