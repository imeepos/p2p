use super::*;
use std::fs;
use std::path::PathBuf;

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("acp-common-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn sample_policy() -> PeerPolicy {
    PeerPolicy {
        scope: Scope::Sandbox,
        allow_mcp: vec!["fs".to_owned()],
        ask_route: AskRoute::RemoteGui,
        note: "first contact".to_owned(),
        granted_at: "2026-09-04T12:00:00Z".to_owned(),
        fingerprint: "ab12cd34".to_owned(),
    }
}

#[test]
fn default_deny_unknown_peer() {
    let table = PolicyTable::new();
    assert_eq!(table.authorize("stranger"), Err(ErrorCode::PeerNotAllowed));
    assert!(table.lookup("stranger").is_none());
}

#[test]
fn grant_authorize_revoke_roundtrip() {
    let mut table = PolicyTable::new();
    table.grant("peerA", sample_policy());
    let policy = table.authorize("peerA").unwrap();
    assert_eq!(policy.scope, Scope::Sandbox);
    assert_eq!(policy.ask_route, AskRoute::RemoteGui);
    assert!(policy.mcp_allowed("fs"));
    assert!(!policy.mcp_allowed("shell"));
    assert_eq!(table.peers().count(), 1);
    assert!(table.revoke("peerA"));
    assert!(!table.revoke("peerA"), "double revoke reports no-op");
    assert_eq!(table.authorize("peerA"), Err(ErrorCode::PeerNotAllowed));
}

#[test]
fn save_load_roundtrip() {
    let dir = tmp_dir("roundtrip");
    let path = dir.join(crate::paths::POLICY_FILE);
    let mut table = PolicyTable::new();
    table.grant("peerA", sample_policy());
    table.save(&path).unwrap();
    assert_eq!(PolicyTable::load(&path).unwrap(), table);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn save_is_atomic_without_tmp_leftover() {
    let dir = tmp_dir("atomic");
    let path = dir.join("policy.json");
    let mut table = PolicyTable::new();
    table.grant("peerA", sample_policy());
    table.save(&path).unwrap();
    let tmp_left: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(tmp_left.is_empty(), "tmp leftovers: {tmp_left:?}");
    assert!(PolicyTable::load(&path).is_ok());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn corrupted_file_is_explicit_error() {
    let dir = tmp_dir("corrupt");
    let path = dir.join("policy.json");
    fs::write(&path, "{ not json").unwrap();
    match PolicyTable::load(&path) {
        Err(PolicyStoreError::Corrupted(_)) => {}
        other => panic!("expected Corrupted, got {other:?}"),
    }
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn missing_file_is_explicit_error() {
    let dir = tmp_dir("missing");
    let err = PolicyTable::load(&dir.join("absent.json")).unwrap_err();
    assert!(matches!(err, PolicyStoreError::Io(ref e) if e.kind() == std::io::ErrorKind::NotFound));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn version_mismatch_is_explicit_error() {
    let dir = tmp_dir("version");
    let path = dir.join("policy.json");
    fs::write(&path, "{\"version\":99,\"peers\":{}}").unwrap();
    assert!(matches!(
        PolicyTable::load(&path),
        Err(PolicyStoreError::UnsupportedVersion(99))
    ));
    fs::remove_dir_all(&dir).unwrap();
}
