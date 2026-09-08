//! WorkspaceStore 单测：加载/种子落盘/增删持久化/legacy 兜底/错误路径。

use std::path::PathBuf;

use super::{WorkspaceStore, WorkspaceStoreError};
use crate::config::WorkspaceDef;

fn tmp_path(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("acp-ws-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("tmp dir");
    dir.join("acp-workspaces.json")
}

fn seed() -> Vec<WorkspaceDef> {
    vec![WorkspaceDef {
        id: "ws1".to_owned(),
        name: "first".to_owned(),
        dir: "/tmp".to_owned(),
    }]
}

#[test]
fn missing_file_seeds_and_persists() {
    let path = tmp_path("seed");
    let store = WorkspaceStore::open(&seed(), None, path.clone()).expect("open");
    assert_eq!(store.rows().len(), 1);
    let raw = std::fs::read_to_string(&path).expect("persisted");
    assert!(raw.contains("\"ws1\""), "file must exist after seeding");
    // 重开读回同一条目（持久化生效）。
    let reopened = WorkspaceStore::open(&[], None, path).expect("reopen");
    assert_eq!(reopened.rows().len(), 1);
}

#[test]
fn corrupt_file_refuses_open() {
    let path = tmp_path("corrupt");
    std::fs::write(&path, "not-json").expect("write junk");
    let err = WorkspaceStore::open(&[], None, path).expect_err("must refuse");
    assert!(matches!(err, WorkspaceStoreError::Corrupt { .. }));
}

#[test]
fn add_rejects_bad_fields_and_duplicate() {
    let store = WorkspaceStore::open(&seed(), None, tmp_path("add")).expect("open");
    assert!(matches!(
        store.add("", "n", "/tmp"),
        Err(WorkspaceStoreError::EmptyField)
    ));
    assert!(matches!(
        store.add("bad id!", "n", "/tmp"),
        Err(WorkspaceStoreError::BadId)
    ));
    assert!(matches!(
        store.add("ws2", "n", "relative/path"),
        Err(WorkspaceStoreError::InvalidDir(_))
    ));
    assert!(matches!(
        store.add("ws2", "n", "/definitely/not/a/dir-xyz"),
        Err(WorkspaceStoreError::InvalidDir(_))
    ));
    assert!(matches!(
        store.add("ws1", "dup", "/tmp"),
        Err(WorkspaceStoreError::DuplicateId(_))
    ));
}

#[test]
fn add_persists_and_remove_roundtrips() {
    let path = tmp_path("roundtrip");
    let store = WorkspaceStore::open(&seed(), None, path.clone()).expect("open");
    let def = store.add("ws2", "second", "/tmp").expect("add");
    assert_eq!(def.id, "ws2");
    let reopened = WorkspaceStore::open(&[], None, path).expect("reopen");
    assert_eq!(reopened.rows().len(), 2);
    reopened.remove("ws2").expect("remove");
    assert!(reopened.resolve(Some("ws2")).is_none());
}

#[test]
fn legacy_default_falls_back_and_is_not_removable() {
    let store = WorkspaceStore::open(&[], Some("/tmp/legacy".to_owned()), tmp_path("legacy"))
        .expect("open");
    let fallback = store.resolve(None).expect("legacy default");
    assert_eq!(fallback.dir, "/tmp/legacy");
    assert!(matches!(
        store.remove("default"),
        Err(WorkspaceStoreError::LegacyDefault)
    ));
    assert!(matches!(
        store.remove("nope"),
        Err(WorkspaceStoreError::Unknown(_))
    ));
}

#[test]
fn explicit_default_shadows_legacy() {
    let store = WorkspaceStore::open(
        &[WorkspaceDef {
            id: "default".to_owned(),
            name: "explicit".to_owned(),
            dir: "/tmp".to_owned(),
        }],
        Some("/tmp/legacy".to_owned()),
        tmp_path("shadow"),
    )
    .expect("open");
    let hit = store.resolve(None).expect("explicit default");
    assert_eq!(hit.dir, "/tmp");
    // 显式占位行可删，删后回落 legacy。
    store.remove("default").expect("remove explicit default");
    assert_eq!(store.resolve(None).expect("legacy").dir, "/tmp/legacy");
}
