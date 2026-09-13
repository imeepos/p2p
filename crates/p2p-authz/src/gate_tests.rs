//! chat PEP 判定核心测试（Amended A-1 红绿锚点）：Allow 静默、四因 reason 码、
//! 读失败=拒、media 旗标选 perm；每次 Deny 恰落一条 authz.denied 审计。

use std::path::{Path, PathBuf};

use super::chat_admit;
use crate::{Authz, SystemClock};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-authz-gate-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn authz(dir: &Path) -> Authz<SystemClock> {
    Authz::new(dir, SystemClock)
}

fn audit_text(dir: &Path) -> String {
    std::fs::read_to_string(crate::audit::audit_path(dir)).unwrap_or_default()
}

/// 四因逐因：NotBound / Expired / BrokenRole / MissingPerm，码逐字、审计逐条。
#[test]
fn deny_reasons_match_waterfall_and_audited() {
    // NotBound：空表。
    let dir = temp_dir("notbound");
    let err = chat_admit(&dir, "peer-a", false).unwrap_err();
    assert_eq!(err.reason, "NotBound");
    let text = audit_text(&dir);
    assert!(text.contains("authz.denied"), "审计须有拒绝事件: {text}");
    assert!(text.contains("chat peer=peer-a perm=chat.send reason=NotBound"));

    // Expired：绑定已过期。
    let dir = temp_dir("expired");
    authz(&dir).bind("peer-b", "friend", Some(1), "").unwrap();
    assert_eq!(
        chat_admit(&dir, "peer-b", false).unwrap_err().reason,
        "Expired"
    );
    assert!(audit_text(&dir).contains("reason=Expired"));

    // BrokenRole：绑定指向不存在角色（防御路径，绕过 bind 的存在性检查直写表）。
    let dir = temp_dir("broken");
    authz(&dir).bind("peer-c", "friend", None, "").unwrap();
    let engine = crate::AuthzEngine::from_parts(
        vec![],
        vec![crate::Binding {
            peer_id: "peer-c".into(),
            role_id: "ghost".into(),
            granted_at: 0,
            note: String::new(),
            expires_at: None,
        }],
    );
    crate::store::save_bindings(&dir, &engine.bindings()).unwrap();
    assert_eq!(
        chat_admit(&dir, "peer-c", false).unwrap_err().reason,
        "BrokenRole"
    );
    assert!(audit_text(&dir).contains("reason=BrokenRole"));

    // MissingPerm：自定义角色不含 chat.send。
    let dir = temp_dir("noperm");
    authz(&dir)
        .create_role("viewer", "viewer", &["a2a.discover".to_string()], "")
        .unwrap();
    authz(&dir).bind("peer-d", "viewer", None, "").unwrap();
    assert_eq!(
        chat_admit(&dir, "peer-d", false).unwrap_err().reason,
        "MissingPerm"
    );
    assert!(audit_text(&dir).contains("reason=MissingPerm"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Allow：内建 friend 绑定后 send/attachment 双闸放行且不落拒绝审计。
#[test]
fn bound_friend_allows_both_gates_without_deny_audit() {
    let dir = temp_dir("allow");
    authz(&dir).bind("peer-ok", "friend", None, "").unwrap();
    assert!(chat_admit(&dir, "peer-ok", false).is_ok());
    assert!(chat_admit(&dir, "peer-ok", true).is_ok());
    assert!(
        !crate::audit::audit_path(&dir).exists(),
        "Allow 不得落拒绝审计"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// media 旗标决定 perm：仅含 chat.send 的角色过 attachment 闸即 MissingPerm。
#[test]
fn media_flag_selects_attachment_permission() {
    let dir = temp_dir("media");
    authz(&dir)
        .create_role("texter", "texter", &["chat.send".to_string()], "")
        .unwrap();
    authz(&dir).bind("peer-e", "texter", None, "").unwrap();
    assert!(chat_admit(&dir, "peer-e", false).is_ok(), "send 闸放行");
    let err = chat_admit(&dir, "peer-e", true).unwrap_err();
    assert_eq!(err.reason, "MissingPerm");
    assert!(
        audit_text(&dir).contains("perm=chat.attachment reason=MissingPerm"),
        "media=true 必须判 chat.attachment: {}",
        audit_text(&dir)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 读失败=拒（红线 2）：损坏绑定表 → ReadFailed，禁止静默放行。
#[test]
fn corrupted_store_reads_deny_not_permit() {
    let dir = temp_dir("corrupt");
    let authz_dir = dir.join("authz");
    std::fs::create_dir_all(&authz_dir).unwrap();
    std::fs::write(authz_dir.join("bindings.json"), "{ not json").unwrap();
    let err = chat_admit(&dir, "peer-f", false).unwrap_err();
    assert_eq!(err.reason, "ReadFailed");
    assert!(audit_text(&dir).contains("reason=ReadFailed"));
    let _ = std::fs::remove_dir_all(&dir);
}
