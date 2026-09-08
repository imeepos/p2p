//! AgentStore 单测（行数红线拆出）。
use crate::a2a::{AgentStore, StoreError};
use a2a::Visibility;
use p2p_identity::Keypair;
use std::path::PathBuf;

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "a2a-agents-test-{tag}-{}.json",
        uuid::Uuid::new_v4().simple()
    ))
}

#[test]
fn create_list_update_remove_roundtrip() {
    let path = temp_path("crud");
    let store = AgentStore::open(path.clone()).unwrap();
    let def = store
        .create(
            Some("code-review".into()),
            "代码评审员".into(),
            "对 PR 做代码评审".into(),
            vec![],
            Visibility::Public,
            1_000,
        )
        .unwrap();
    assert_eq!(def.agent_id, "code-review");
    assert_eq!(store.list().len(), 1);
    store
        .set_visibility("code-review", Visibility::Private)
        .unwrap();
    assert_eq!(
        store.get("code-review").unwrap().visibility,
        Visibility::Private
    );
    store.set_enabled("code-review", false).unwrap();
    assert!(store.network_defs().is_empty(), "停用即不出卡");
    store.remove("code-review").unwrap();
    assert!(store.list().is_empty());
    assert!(store.remove("code-review").is_err());
    let _ = std::fs::remove_file(path);
}

#[test]
fn duplicate_rejected_and_persist_reopens() {
    let path = temp_path("dup");
    let store = AgentStore::open(path.clone()).unwrap();
    store
        .create(None, "n".into(), "d".into(), vec![], Visibility::Public, 1)
        .unwrap();
    let id = store.list()[0].agent_id.clone();
    assert!(matches!(
        store.create(
            Some(id),
            "n".into(),
            "d".into(),
            vec![],
            Visibility::Public,
            1
        ),
        Err(StoreError::Duplicate(_))
    ));
    // 重开恢复
    drop(store);
    let store = AgentStore::open(path.clone()).unwrap();
    assert_eq!(store.list().len(), 1);
    let _ = std::fs::remove_file(path);
}

#[test]
fn signed_cards_visibility_filter() {
    let path = temp_path("vis");
    let store = AgentStore::open(path.clone()).unwrap();
    store
        .create(
            Some("pub-a".into()),
            "a".into(),
            "d".into(),
            vec![],
            Visibility::Public,
            1,
        )
        .unwrap();
    store
        .create(
            Some("priv-b".into()),
            "b".into(),
            "d".into(),
            vec![],
            Visibility::Private,
            1,
        )
        .unwrap();
    let kp = Keypair::generate();
    let host = kp.peer_id().to_string();
    let now = 10_000;
    // owner 全见
    assert_eq!(store.signed_cards_for(&kp, &host, true, &[], now).len(), 2);
    // 远程仅 public（fail-closed：无授权清单时）
    let cards = store.signed_cards_for(&kp, &host, false, &[], now);
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].0.payload.agent_id, "pub-a");
    // 授权清单内 private 对该 peer 可见
    let cards = store.signed_cards_for(&kp, &host, false, &["priv-b".to_owned()], now);
    assert_eq!(cards.len(), 2);
    for card in &cards {
        card.verify(now).unwrap();
    }
    let _ = std::fs::remove_file(path);
}
