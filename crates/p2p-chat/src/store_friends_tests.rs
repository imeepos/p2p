//! store_friends 单元测试（Y1）：双 Doc 并发 add 合并、remove tombstone、
//! 旧 JSON 迁移兼容、快照持久化 roundtrip、损坏行容错（单元测试，非 tests/ 目录）。

use std::path::PathBuf;

use crate::model::ChatFriend;
use crate::store::Store;
use crate::store_friends::FriendsBook;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-chat-yrs-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn book_path(tag: &str) -> PathBuf {
    let path = temp_dir(tag).join("chat/friends.json");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("chat dir");
    path
}

fn friend(peer: &str, nickname: &str) -> ChatFriend {
    ChatFriend {
        peer_id: peer.to_string(),
        nickname: nickname.to_string(),
        addrs: Vec::new(),
        note: None,
        group: None,
    }
}

fn peer_ids(list: &[ChatFriend]) -> Vec<&str> {
    list.iter().map(|f| f.peer_id.as_str()).collect()
}

/// 双 Doc 并发 add 互见合并：b 基于 a 写入前的旧基线追加，日志合并后全量保留，
/// 全程无文件锁（CRDT 合并语义替代 R1 串行化）。
#[test]
fn concurrent_adds_from_stale_bases_both_preserved_without_lock() {
    let path = book_path("stale");
    let a = FriendsBook::load(&path).expect("load a");
    let b = FriendsBook::load(&path).expect("load b（与 a 同基线，模拟并发）");
    a.upsert(&path, friend("aaa", "a")).expect("a add");
    b.upsert(&path, friend("bbb", "b")).expect("b add（基于旧基线）");
    let merged = FriendsBook::load(&path).expect("load c").list();
    assert_eq!(
        peer_ids(&merged),
        vec!["aaa", "bbb"],
        "并发追加必须全量合并: {merged:?}"
    );
}

/// 并发 remove 与 add：tombstone 生效移除目标好友，另一进程新增的好友保留。
#[test]
fn concurrent_remove_and_add_merge_per_yrs_semantics() {
    let path = book_path("rmadd");
    let seed = FriendsBook::load(&path).expect("seed");
    seed.upsert(&path, friend("aaa", "a")).expect("seed add");
    let a = FriendsBook::load(&path).expect("a");
    let b = FriendsBook::load(&path).expect("b（同基线）");
    assert!(a.remove(&path, "aaa").expect("a remove"));
    b.upsert(&path, friend("bbb", "b")).expect("b add");
    let merged = FriendsBook::load(&path).expect("merge").list();
    assert_eq!(
        peer_ids(&merged),
        vec!["bbb"],
        "remove(tombstone) 与并发 add 各自生效: {merged:?}"
    );
}

/// tombstone 跨重载生效；remove 幂等返回 false；同 peer re-add 覆盖 tombstone。
#[test]
fn remove_tombstone_survives_reload_and_readd_works() {
    let path = book_path("tomb");
    let a = FriendsBook::load(&path).expect("load");
    a.upsert(&path, friend("aaa", "a")).expect("add");
    assert!(a.remove(&path, "aaa").expect("remove"));
    let gone = FriendsBook::load(&path).expect("reload").list();
    assert!(gone.is_empty(), "tombstone 跨重载生效: {gone:?}");
    assert!(!a.remove(&path, "aaa").expect("remove again"), "不在簿 false");
    let b = FriendsBook::load(&path).expect("reload b");
    b.upsert(&path, friend("aaa", "a2")).expect("re-add");
    let back = FriendsBook::load(&path).expect("reload c").list();
    assert_eq!(back.len(), 1, "re-add 覆盖 tombstone");
    assert_eq!(back[0].nickname, "a2");
}

/// 旧 JSON 数组迁移：原文件备份、内容进 doc、迁移后可继续写。
#[test]
fn legacy_json_array_migrates_and_backs_up() {
    let path = book_path("legacy");
    let legacy = r#"[{"peerId":"pp1","nickname":"旧名","addrs":["/ip4/1.2.3.4/tcp/1"],"note":null}]"#;
    std::fs::write(&path, legacy).expect("seed legacy");
    let book = FriendsBook::load(&path).expect("load migrates");
    let list = book.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].nickname, "旧名");
    assert_eq!(list[0].group, None, "旧记录缺 group 读回未分组");
    let backup = std::fs::read_dir(path.parent().unwrap())
        .expect("dir")
        .filter_map(Result::ok)
        .find(|e| e.file_name().to_string_lossy().starts_with("friends.json.bak-yrs-"))
        .expect("备份存在");
    assert_eq!(
        std::fs::read_to_string(backup.path()).expect("backup"),
        legacy,
        "备份内容 == 迁移前原文件"
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("raw").lines().count(),
        2,
        "头行 + 全量快照行"
    );
    book.upsert(&path, friend("pp2", "新")).expect("迁移后 add");
    assert_eq!(
        FriendsBook::load(&path).expect("reload").list().len(),
        2,
        "迁移后继续可用"
    );
}

/// 快照持久化 roundtrip：多笔变更后新实例读回一致；损坏行跳过不静默丢簿。
#[test]
fn snapshot_roundtrip_and_corrupt_line_tolerance() {
    let path = book_path("round");
    let a = FriendsBook::load(&path).expect("load");
    a.upsert(&path, friend("aaa", "a")).expect("add a");
    a.upsert(&path, friend("bbb", "b")).expect("add b");
    a.upsert(&path, friend("aaa", "a2")).expect("upsert a");
    a.remove(&path, "bbb").expect("remove b");
    let view = FriendsBook::load(&path).expect("reload").list();
    assert_eq!(view.len(), 1);
    assert_eq!(view[0].nickname, "a2");
    let raw = std::fs::read_to_string(&path).expect("raw");
    let mut lines: Vec<String> = raw.lines().map(String::from).collect();
    lines.push("{\"u\":\"!!!not-base64!\"}".into());
    std::fs::write(&path, lines.join("\n")).expect("追加损坏行");
    let after = FriendsBook::load(&path).expect("reload corrupt").list();
    assert_eq!(peer_ids(&after), vec!["aaa"], "损坏行跳过，其余保留");
}

/// Store 门面等价双进程场景（回归旧 store_lock 并发用例）：两实例先后写、
/// 新实例读回全量，锁已退役仍零丢失。
#[test]
fn two_stores_upserts_merge_without_lock() {
    let dir = temp_dir("store");
    let a = Store::new(dir.clone()).expect("store a");
    let b = Store::new(dir.clone()).expect("store b");
    a.upsert_friend(friend("4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi", "s1"))
        .expect("a add");
    b.upsert_friend(friend("8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR", "s2"))
        .expect("b add");
    let book = Store::new(dir.clone())
        .expect("store c")
        .friends_list()
        .expect("list");
    assert_eq!(book.len(), 2, "无锁写全量保留: {book:?}");
}
