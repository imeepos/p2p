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

/// W2b 补料接线（红）：文件缺失 = `wirable_book` 返回 None（不接线、不报错）。
#[test]
fn wirable_book_absent_file_is_none() {
    let dir = temp_root("wire-none");
    assert_eq!(wirable_book(&dir), None, "缺失 = 不接线");
}

/// W2b 补料接线（红）：损坏文件 = None 且不 panic（warn 由 p2p 层留观测）。
#[test]
fn wirable_book_corrupt_file_is_none() {
    let dir = temp_root("wire-bad");
    fs::write(dir.join(FILE_NAME), b"{not json").expect("write ok");
    assert_eq!(wirable_book(&dir), None, "损坏 = 不接线");
}

/// W2b 补料接线（绿）：命令面写盘后 `wirable_book` 即产出同一路径——
/// 证明"命令面写的簿"正是"装配读的簿"（同根同名，防假功能回归）。
#[test]
fn wirable_book_sees_command_write_path() {
    let dir = temp_root("wire-ok");
    upsert_peer(
        &dir.join(FILE_NAME),
        "peer-a".into(),
        vec!["10.0.0.1/u4000".into()],
        "wired".into(),
    )
    .expect("命令面 upsert ok");
    assert_eq!(
        wirable_book(&dir),
        Some(dir.join(FILE_NAME)),
        "接线路径 = 命令面写盘路径"
    );
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
