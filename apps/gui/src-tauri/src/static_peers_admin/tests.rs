//! static_peers_admin 单测（红绿双向）：upsert 去重、remove 幂等、缺失/损坏
//! 读写语义、camelCase 形状。内核直测（&Path），不经 Tauri State。

use std::fs;
use std::path::PathBuf;

use super::*;

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-console-spa-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("创建临时目录");
    dir
}

#[test]
fn list_missing_file_is_empty_report() {
    let dir = temp_root("missing");
    assert_eq!(list_peers(&dir.join(FILE_NAME)).unwrap(), Vec::new());
}

#[test]
fn list_corrupt_file_is_explicit_error() {
    let dir = temp_root("corrupt");
    fs::write(dir.join(FILE_NAME), b"{not json").expect("write ok");
    let err = list_peers(&dir.join(FILE_NAME)).unwrap_err();
    assert!(err.contains("static-peers.json"), "{err}");
}

#[test]
fn upsert_dedupes_by_peer_id_and_persists() {
    let dir = temp_root("dedupe");
    let path = dir.join(FILE_NAME);
    upsert_peer(
        &path,
        "peer-a".into(),
        vec!["10.0.0.1/u4000".into()],
        "first".into(),
    )
    .expect("upsert ok");
    upsert_peer(
        &path,
        "peer-a".into(),
        vec!["10.0.0.9/u4999".into()],
        "moved".into(),
    )
    .expect("重复 upsert ok");
    upsert_peer(&path, "peer-b".into(), vec![], "".into()).expect("upsert ok");
    let peers = list_peers(&path).unwrap();
    assert_eq!(peers.len(), 2, "同 peerId 去重不追加");
    let a = peers.iter().find(|p| p.peer_id == "peer-a").unwrap();
    assert_eq!(a.addrs, vec!["10.0.0.9/u4999"]);
    assert_eq!(a.note, "moved");
    assert_eq!(
        serde_json::to_value(&peers[0]).unwrap()["peerId"],
        "peer-a",
        "camelCase 形状逐字"
    );
}

#[test]
fn remove_is_idempotent() {
    let dir = temp_root("remove");
    let path = dir.join(FILE_NAME);
    upsert_peer(&path, "peer-a".into(), vec![], "".into()).expect("upsert ok");
    remove_peer(&path, "peer-a").expect("remove existing ok");
    assert!(list_peers(&path).unwrap().is_empty());
    remove_peer(&path, "peer-a").expect("remove missing 亦 Ok（幂等）");
    remove_peer(&path, "never-existed").expect("remove missing 亦 Ok");
}

#[test]
fn remove_on_missing_file_creates_empty_book() {
    let dir = temp_root("remove-missing");
    let path = dir.join(FILE_NAME);
    remove_peer(&path, "ghost").expect("空册移除 ok");
    assert!(list_peers(&path).unwrap().is_empty());
}

#[test]
fn upsert_blank_peer_id_is_rejected() {
    let dir = temp_root("blank-id");
    let err = upsert_peer(&dir.join(FILE_NAME), "  ".into(), vec![], "".into()).unwrap_err();
    assert!(err.contains("peerId"), "{err}");
}
