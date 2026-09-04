use p2p::Node;
use p2p_chat::{Chat, ChatKind, ChatMediaInput};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("p2p-chat-t34-{tag}-{}", uuid::Uuid::new_v4()))
}

async fn chat_at(dir: &Path) -> (Arc<Node>, Chat) {
    let node_dir = dir.join("node");
    let node = Arc::new(
        Node::builder()
            .mdns(false)
            .quic_port(0)
            .tcp_port(0)
            .data_dir(node_dir)
            .build()
            .await
            .expect("node builds on ephemeral ports"),
    );
    let chat = Chat::new(node.clone(), dir.to_path_buf()).expect("chat store builds");
    (node, chat)
}

fn envelope(id: &str, ts: i64) -> serde_json::Value {
    serde_json::json!({"id": id, "peer": "11111111111111111111111111111111", "sender": "me", "kind": "text", "tsMs": ts, "text": "message", "media": null, "status": "delivered"})
}

#[tokio::test]
async fn peer_id_rejects_invalid_length_and_self() {
    let dir = temp_dir("peer");
    let (node, chat) = chat_at(&dir).await;
    let invalid = chat
        .friend_add("not base58!", "x", Vec::new(), None)
        .expect_err("invalid PeerId must fail");
    assert!(
        invalid.to_string().contains("PeerId"),
        "invalid peer error is explicit: {invalid}"
    );
    let wrong = chat
        .friend_add("hello", "x", Vec::new(), None)
        .expect_err("wrong-length PeerId must fail");
    assert!(
        wrong.to_string().contains("非法"),
        "wrong length error is explicit: {wrong}"
    );
    let self_id = node.local_peer_id().to_string();
    let self_err = chat
        .friend_add(&self_id, "x", Vec::new(), None)
        .expect_err("self PeerId must fail");
    assert!(
        self_err.to_string().contains("自己"),
        "self-peer error is explicit: {self_err}"
    );
    node.shutdown();
    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn jsonl_empty_no_newline_corrupt_and_duplicate_are_observable() {
    let dir = temp_dir("jsonl");
    let peer = "11111111111111111111111111111111";
    let path = dir.join("chat/messages").join(format!("{peer}.jsonl"));
    fs::create_dir_all(path.parent().expect("messages parent")).expect("create messages");
    fs::write(&path, "").expect("write empty JSONL");
    let (node, chat) = chat_at(&dir).await;
    assert!(
        chat.history(peer, None, 10)
            .expect("empty JSONL reads")
            .is_empty(),
        "empty file yields empty history"
    );
    fs::write(
        &path,
        serde_json::to_string(&envelope("one", 1)).expect("serialize"),
    )
    .expect("write no-newline JSONL");
    let (node2, chat2) = chat_at(&dir).await;
    assert_eq!(
        chat2
            .history(peer, None, 10)
            .expect("no newline reads")
            .len(),
        1,
        "last line need not end newline"
    );
    drop(chat2);
    node2.shutdown();
    node.shutdown();
    fs::write(
        &path,
        format!(
            "{}\ncorrupt\n{}\n{}",
            serde_json::to_string(&envelope("one", 1)).expect("serialize"),
            serde_json::to_string(&envelope("two", 2)).expect("serialize"),
            serde_json::to_string(&envelope("two", 2)).expect("serialize")
        ),
    )
    .expect("write corrupt duplicate JSONL");
    let (node3, chat3) = chat_at(&dir).await;
    let rows = chat3
        .history(peer, None, 10)
        .expect("corrupt line is skipped");
    assert_eq!(
        rows.len(),
        2,
        "corrupt line skipped and same-id lines collapse"
    );
    assert_eq!(
        rows.iter().filter(|m| m.id == "two").count(),
        1,
        "same-id lines collapse to last-wins"
    );
    node3.shutdown();
    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn same_id_status_progression_collapses_to_latest() {
    let dir = temp_dir("dupstatus");
    let peer = "11111111111111111111111111111111";
    let path = dir.join("chat/messages").join(format!("{peer}.jsonl"));
    fs::create_dir_all(path.parent().expect("messages parent")).expect("create messages");
    let mut pending = envelope("dup-1", 10);
    pending["status"] = serde_json::json!("pending");
    let mut delivered = envelope("dup-1", 10);
    delivered["status"] = serde_json::json!("delivered");
    fs::write(
        &path,
        format!(
            "{}\n{}\n",
            serde_json::to_string(&pending).expect("serialize"),
            serde_json::to_string(&delivered).expect("serialize")
        ),
    )
    .expect("write status progression JSONL");
    let (node, chat) = chat_at(&dir).await;
    let rows = chat.history(peer, None, 10).expect("history reads");
    assert_eq!(rows.len(), 1, "same id collapses to single row");
    assert_eq!(
        rows[0].status,
        p2p_chat::ChatStatus::Delivered,
        "last-wins keeps the final status"
    );
    node.shutdown();
    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn pagination_limits_and_cursor_errors_are_explicit() {
    let dir = temp_dir("pages");
    let peer = "11111111111111111111111111111111";
    let path = dir.join("chat/messages").join(format!("{peer}.jsonl"));
    fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    let lines = (0..101)
        .map(|i| serde_json::to_string(&envelope(&format!("id-{i}"), i)).expect("serialize"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&path, lines).expect("write pages");
    let (node, chat) = chat_at(&dir).await;
    assert_eq!(
        chat.history(peer, None, 0)
            .expect("limit zero defaults")
            .len(),
        50,
        "limit 0 uses default 50"
    );
    assert_eq!(chat.history(peer, None, 1).expect("limit one").len(), 1);
    assert_eq!(chat.history(peer, None, 100).expect("limit 100").len(), 100);
    assert_eq!(
        chat.history(peer, None, 101).expect("limit 101 caps").len(),
        100
    );
    assert!(
        chat.history(peer, Some("missing"), 1).is_err(),
        "unknown cursor must fail"
    );
    assert!(
        chat.history("bad", None, 1).is_err(),
        "invalid history peer must fail"
    );
    node.shutdown();
    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn atomic_write_failure_returns_io_signal() {
    let dir = temp_dir("atomic");
    let friends = dir.join("chat/friends.json");
    fs::create_dir_all(&friends).expect("friends directory");
    let (node, chat) = chat_at(&dir).await;
    let other = p2p_identity::Keypair::generate().peer_id().to_string();
    let err = chat
        .friend_add(&other, "atomic", Vec::new(), None)
        .expect_err("directory target makes atomic write fail");
    assert!(
        err.to_string().contains("IO") || err.to_string().contains("目录"),
        "atomic failure is observable: {err}"
    );
    node.shutdown();
    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn attachment_boundaries_and_sanitized_path_are_checked() {
    let dir = temp_dir("media");
    let (node, chat) = chat_at(&dir).await;
    let peer = p2p_identity::Keypair::generate().peer_id().to_string();
    let empty = chat
        .send(
            &peer,
            ChatKind::File,
            None,
            Some(ChatMediaInput {
                name: "x".into(),
                mime: "application/octet-stream".into(),
                data: Vec::new(),
            }),
            None,
        )
        .await
        .expect_err("zero bytes must fail");
    assert!(
        empty.to_string().contains("为空"),
        "zero-byte error explicit: {empty}"
    );
    let report = chat
        .send(
            &peer,
            ChatKind::File,
            None,
            Some(ChatMediaInput {
                name: r"..\bad/名.bin".into(),
                mime: "application/octet-stream".into(),
                data: vec![7],
            }),
            None,
        )
        .await
        .expect("small attachment stores before delivery");
    assert_eq!(
        report.message.media.as_ref().expect("media metadata").size,
        1
    );
    assert!(
        report
            .message
            .media
            .as_ref()
            .expect("media metadata")
            .path
            .as_ref()
            .expect("path")
            .contains("..bad"),
        "sanitized path retained safely"
    );
    node.shutdown();
    let _ = fs::remove_dir_all(dir);
}
