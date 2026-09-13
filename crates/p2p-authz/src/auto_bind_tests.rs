//! auto_bind 共享实现测试：与 CLI 原实现行为逐一对齐（配置读取在调用方，
//! 本层只测空配置/跳过/绑定/降级四路径与审计落点）。

use std::path::{Path, PathBuf};

use super::{auto_bind_default_role, AutoBind};
use crate::store;
use crate::{Authz, SystemClock};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-authz-autobind-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn audit_text(dir: &Path) -> String {
    std::fs::read_to_string(crate::audit::audit_path(dir)).unwrap_or_default()
}

/// 缺省 friend（空配置视为 Disabled，由调用方传入解析后的缺省值）。
#[test]
fn binds_default_role_with_audit() {
    let dir = temp_dir("bind");
    let out = auto_bind_default_role(&dir, "peer-1", "friend");
    assert_eq!(
        out,
        AutoBind::Bound {
            role_id: "friend".to_owned()
        }
    );
    let bindings = store::load_bindings(&dir).unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].role_id, "friend");
    assert_eq!(bindings[0].note, "auto: default_role");
    assert!(audit_text(&dir).contains("authz.bound"), "绑定须落审计");
    assert!(out.note().unwrap().contains("friend"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// 既有绑定（含更高角色）跳过不覆写，且不产生新审计事件。
#[test]
fn existing_binding_is_skipped_not_overwritten() {
    let dir = temp_dir("skip");
    Authz::new(&dir, SystemClock)
        .bind("peer-2", "ally", None, "")
        .unwrap();
    let before = audit_text(&dir).lines().count();
    let out = auto_bind_default_role(&dir, "peer-2", "friend");
    assert_eq!(
        out,
        AutoBind::SkippedAlreadyBound {
            role_id: "ally".to_owned()
        }
    );
    let bindings = store::load_bindings(&dir).unwrap();
    assert_eq!(bindings[0].role_id, "ally", "既有绑定保持不变");
    assert_eq!(audit_text(&dir).lines().count(), before, "跳过零新增审计");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 空串 = 显式禁用：零动作零落账。
#[test]
fn empty_role_disables() {
    let dir = temp_dir("disabled");
    let out = auto_bind_default_role(&dir, "peer-3", "");
    assert_eq!(out, AutoBind::Disabled);
    assert_eq!(out.note(), None, "显式禁用不出警告文案");
    assert!(!crate::audit::audit_path(&dir).exists(), "禁用无任何落账");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 角色不存在降级警告：无绑定、无 bound 审计。
#[test]
fn missing_role_degrades_to_warning() {
    let dir = temp_dir("warn");
    let out = auto_bind_default_role(&dir, "peer-4", "ghost");
    let AutoBind::Warned { reason } = out.clone() else {
        panic!("角色不存在必须降级为警告: {out:?}");
    };
    assert!(reason.contains("ghost"), "警告携带角色 id: {reason}");
    assert!(
        store::load_bindings(&dir).unwrap().is_empty(),
        "降级不产生绑定"
    );
    assert!(
        !audit_text(&dir).contains("authz.bound"),
        "降级不落 bound 事件"
    );
    assert!(out.note().unwrap().starts_with("警告"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// 损坏绑定表 → 读失败降级警告（判定面另有读失败=拒兜底）。
#[test]
fn corrupted_bindings_degrade_to_warning() {
    let dir = temp_dir("corrupt");
    let authz_dir = dir.join("authz");
    std::fs::create_dir_all(&authz_dir).unwrap();
    std::fs::write(authz_dir.join("bindings.json"), "{ not json").unwrap();
    assert!(matches!(
        auto_bind_default_role(&dir, "peer-5", "friend"),
        AutoBind::Warned { .. }
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
