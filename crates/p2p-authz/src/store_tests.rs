//! 存储三测（验收第 5 项）：原子写回读一致 / 损坏文件显式报错（不静默回退空表）/
//! 版本不符报 UnsupportedVersion；另覆盖缺失文件首用空表与 .tmp 无残留。

use std::path::PathBuf;

use crate::binding::Binding;
use crate::role::builtin_roles;
use crate::store::{
    bindings_path, load_bindings, load_roles, roles_path, save_bindings, save_roles,
    AuthzStoreError, FILE_VERSION,
};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-authz-store-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn sample_binding(peer_id: &str, role_id: &str) -> Binding {
    Binding {
        peer_id: peer_id.to_owned(),
        role_id: role_id.to_owned(),
        granted_at: 1_000,
        note: "n".to_owned(),
        expires_at: None,
    }
}

#[test]
fn atomic_save_roundtrip_matches_in_memory_state() {
    let dir = temp_dir("roundtrip");
    let roles = builtin_roles();
    save_roles(&dir, &roles).unwrap();
    let bindings = vec![
        sample_binding("peer-a", "friend"),
        sample_binding("peer-b", "ally"),
    ];
    save_bindings(&dir, &bindings).unwrap();

    assert_eq!(load_roles(&dir).unwrap(), roles);
    assert_eq!(load_bindings(&dir).unwrap(), bindings);
    assert!(!roles_path(&dir).with_extension("json.tmp").exists());
    assert!(!bindings_path(&dir).with_extension("json.tmp").exists());

    // 信封逐字检查：version=1 + 命名数组（设计 §6 形状）。
    let text = std::fs::read_to_string(roles_path(&dir)).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["version"], FILE_VERSION);
    assert!(value["roles"].is_array());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn corrupt_file_is_explicit_error_not_silent_empty() {
    let dir = temp_dir("corrupt");
    std::fs::create_dir_all(roles_path(&dir).parent().unwrap()).unwrap();
    std::fs::write(roles_path(&dir), "{ not json").unwrap();
    std::fs::write(bindings_path(&dir), "{\"version\":1,\"bindings\":").unwrap();

    let roles = load_roles(&dir);
    assert!(
        matches!(roles, Err(AuthzStoreError::Corrupted(_))),
        "损坏必须显式报错，实得: {roles:?}"
    );
    assert!(matches!(
        load_bindings(&dir),
        Err(AuthzStoreError::Corrupted(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn version_mismatch_is_unsupported_version_error() {
    let dir = temp_dir("version");
    std::fs::create_dir_all(roles_path(&dir).parent().unwrap()).unwrap();
    std::fs::write(roles_path(&dir), "{\"version\":999,\"roles\":[]}").unwrap();
    std::fs::write(
        bindings_path(&dir),
        "{\"version\":2,\"bindings\":[{\"peer_id\":\"p\",\"role_id\":\"friend\",\"granted_at\":1}]}",
    )
    .unwrap();

    assert!(matches!(
        load_roles(&dir),
        Err(AuthzStoreError::UnsupportedVersion(999))
    ));
    assert!(matches!(
        load_bindings(&dir),
        Err(AuthzStoreError::UnsupportedVersion(2))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_files_are_empty_first_use() {
    let dir = temp_dir("missing");
    assert!(load_roles(&dir).unwrap().is_empty());
    assert!(load_bindings(&dir).unwrap().is_empty());
    save_roles(&dir, &[]).unwrap();
    assert!(load_roles(&dir).unwrap().is_empty(), "空表写回仍为空");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_role_key_in_file_is_corrupted_error() {
    let dir = temp_dir("unknown-perm");
    std::fs::create_dir_all(roles_path(&dir).parent().unwrap()).unwrap();
    std::fs::write(
        roles_path(&dir),
        "{\"version\":1,\"roles\":[{\"role_id\":\"x\",\"name\":\"x\",\
         \"permissions\":[\"owner.superuser\"],\"builtin\":false}]}",
    )
    .unwrap();
    assert!(
        matches!(load_roles(&dir), Err(AuthzStoreError::Corrupted(_))),
        "表外权限 key 反序列化必须失败（闭集不被文件绕过）"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
