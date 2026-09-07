use super::*;

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("p2pcli-allow-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn peer(byte: u8) -> String {
    bs58::encode([byte; 32]).into_string()
}

#[test]
fn roundtrip_and_upsert_semantics() {
    let dir = temp_dir("roundtrip");
    let file = dir.join(FILE_NAME);
    let p = peer(1);
    let mut list = AllowlistFile::new();
    assert!(list.upsert(&p, vec!["gpt-4o".into()], "n", None, None, "2026-09-04T00:00:00Z"));
    assert!(!list.upsert(&p, vec![], "", None, None, "2026-09-04T01:00:00Z"));
    save(&file, &list).unwrap();
    assert!(!file.with_extension("json.tmp").exists());
    let loaded = load_or_empty(&file).unwrap();
    assert_eq!(loaded.v, FORMAT_VERSION);
    assert_eq!(loaded.entries[&p].models, Vec::<String>::new());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_file_loads_empty_and_corrupt_errors() {
    let dir = temp_dir("missing");
    let file = dir.join(FILE_NAME);
    assert!(load_or_empty(&file).unwrap().entries.is_empty());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&file, "{ not json").unwrap();
    assert!(load_or_empty(&file).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deny_missing_entry_is_explicit_error() {
    let dir = temp_dir("deny");
    let p = peer(2);
    assert!(deny(dir.to_str().unwrap(), &p).is_err());
    allow(dir.to_str().unwrap(), &p, &[], None, None, None, "2026-09-04T00:00:00Z").unwrap();
    assert!(deny(dir.to_str().unwrap(), &p).unwrap().removed);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn allow_validates_peer_and_models() {
    let dir = temp_dir("validate");
    assert!(allow(dir.to_str().unwrap(), "bad-peer", &[], None, None, None, "t").is_err());
    let p = peer(3);
    assert!(allow(dir.to_str().unwrap(), &p, &["  ".to_owned()], None, None, None, "t").is_err());
    let report = allow(
        dir.to_str().unwrap(),
        &p,
        &[" gpt-4o ".to_owned(), "gpt-4o".to_owned()],
        Some("nb"),
        None,
        None,
        "t",
    )
    .unwrap();
    assert_eq!(report.models, vec!["gpt-4o".to_owned()]);
    assert_eq!(report.source, None);
}

#[test]
fn list_reports_entries_in_stable_order() {
    let dir = temp_dir("list");
    let a = peer(4);
    let b = peer(5);
    allow(dir.to_str().unwrap(), &b, &[], None, None, None, "t").unwrap();
    allow(dir.to_str().unwrap(), &a, &[], None, None, None, "t").unwrap();
    let report = list(dir.to_str().unwrap()).unwrap();
    assert_eq!(report.peers.len(), 2);
    assert_eq!(report.peers[0].peer_id, a, "BTreeMap 序稳定输出");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn v1_archive_without_new_fields_loads_compatibly() {
    let dir = temp_dir("v1compat");
    let file = dir.join(FILE_NAME);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let p = peer(6);
    let legacy = format!(
        r#"{{"v":1,"entries":{{"{}":{{"models":["gpt-4o"],"note":"old","granted_at":"t"}}}}}}"#,
        p
    );
    std::fs::write(&file, legacy).unwrap();
    let loaded = load_or_empty(&file).unwrap();
    let entry = &loaded.entries[&p];
    assert_eq!(entry.source, None, "v1 旧档无 source 字段兼容读");
    assert_eq!(entry.expires_at, None);
    assert_eq!(entry.models, vec!["gpt-4o"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn share_fields_roundtrip_through_upsert() {
    let dir = temp_dir("sharefields");
    let p = peer(7);
    let report = allow(
        dir.to_str().unwrap(),
        &p,
        &["gpt-4o".to_owned()],
        Some("share"),
        Some("share:abc"),
        Some(1_999),
        "t",
    )
    .unwrap();
    assert_eq!(report.source.as_deref(), Some("share:abc"));
    assert_eq!(report.expires_at, Some(1_999));
    let listed = list(dir.to_str().unwrap()).unwrap();
    assert_eq!(listed.peers[0].source.as_deref(), Some("share:abc"));
    assert_eq!(listed.peers[0].expires_at, Some(1_999));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn remove_by_source_only_touches_share_entries() {
    let dir = temp_dir("removebysource");
    let share_peer = peer(8);
    let manual_peer = peer(9);
    allow(
        dir.to_str().unwrap(),
        &share_peer,
        &[],
        None,
        Some("share:share-1"),
        Some(1_999),
        "t",
    )
    .unwrap();
    allow(dir.to_str().unwrap(), &manual_peer, &[], None, None, None, "t").unwrap();
    assert_eq!(remove_by_source(dir.to_str().unwrap(), "share:share-1").unwrap(), 1);
    let report = list(dir.to_str().unwrap()).unwrap();
    assert_eq!(report.peers.len(), 1, "手工条目不受级联");
    assert_eq!(report.peers[0].peer_id, manual_peer);
    assert_eq!(remove_by_source(dir.to_str().unwrap(), "share:missing").unwrap(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}
