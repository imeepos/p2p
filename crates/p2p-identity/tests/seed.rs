//! 种子落盘/加载的行为测试：持久性、权限收紧、非法文件拒绝。

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use p2p_identity::{load_or_generate_seed, load_seed, save_seed, Keypair};

fn unique_path(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("p2p-identity-{tag}-{}-{nanos}.seed", std::process::id()))
}

#[test]
fn save_then_load_restores_identity() {
    let path = unique_path("roundtrip");
    let keypair = Keypair::generate();
    save_seed(&path, &keypair).expect("save seed");

    let loaded = load_seed(&path).expect("load seed");
    assert_eq!(keypair.public(), loaded.public());
    assert_eq!(keypair.peer_id(), loaded.peer_id());
    let _ = fs::remove_file(&path);
}

#[test]
fn load_or_generate_is_stable_across_calls() {
    let path = unique_path("stable");
    let first = load_or_generate_seed(&path).expect("first call generates");
    let second = load_or_generate_seed(&path).expect("second call loads");
    assert_eq!(first.peer_id(), second.peer_id());
    let _ = fs::remove_file(&path);
}

#[test]
fn saved_file_has_private_permissions() {
    let path = unique_path("perm");
    save_seed(&path, &Keypair::generate()).expect("save seed");
    let mode = fs::metadata(&path).expect("meta").permissions().mode();
    assert_eq!(mode & 0o777, 0o600, "seed file must be 0600");
    let _ = fs::remove_file(&path);
}

#[test]
fn load_tightens_loose_permissions() {
    let path = unique_path("tighten");
    save_seed(&path, &Keypair::generate()).expect("save seed");
    let mut perms = fs::metadata(&path).expect("meta").permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&path, perms).expect("chmod");

    let loaded = load_seed(&path).expect("load seed");
    assert!(loaded.public().len() == 32);
    let mode = fs::metadata(&path).expect("meta").permissions().mode();
    assert_eq!(mode & 0o777, 0o600, "load must tighten to 0600");
    let _ = fs::remove_file(&path);
}

#[test]
fn load_rejects_wrong_length() {
    let path = unique_path("short");
    fs::write(&path, b"too-short").expect("write junk");
    let err = match load_seed(&path) {
        Ok(_) => panic!("short file must be rejected"),
        Err(e) => e,
    };
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    let _ = fs::remove_file(&path);
}