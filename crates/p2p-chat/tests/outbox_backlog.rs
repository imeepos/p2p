//! 回归（flush 纪律边界）：ConnectFailed 不消耗重投机会；批量上限下积压多轮收敛。

use p2p_chat::ChatKind;

mod common;
use common::{add_each_other, cleanup, outbox_lines, peer_str, spawn, spawn_at, wait_until};

#[tokio::test]
async fn failed_entries_survive_drain_while_peer_offline() {
    // ConnectFailed 不消耗重投机会（outbox.rs flush 纪律）：对端离线期间的 drain
    // 不把条目死信出队，必须原样保留待对端回归。
    let a = spawn("capa-a").await;
    let b = spawn("capa-b").await;
    add_each_other(&a, &b).await;
    let b_peer = peer_str(&b.node);
    b.node.shutdown();

    let report = a
        .chat
        .send(
            &b_peer,
            ChatKind::Text,
            Some("while-down".into()),
            None,
            None,
        )
        .await
        .expect("send accepted");
    assert!(!report.delivered);

    for _ in 0..2 {
        let r = a
            .chat
            .drain_peer(&b_peer, std::time::Duration::from_secs(2))
            .await;
        assert!(
            r.is_err(),
            "drain against offline peer must report connect failure"
        );
    }
    let rows = a.chat.history(&b_peer, None, 10).expect("history");
    assert_eq!(rows.len(), 1, "entry survives offline drains");
    cleanup(&a);
}

fn failed_line(i: usize, b_peer: &str) -> String {
    format!(
        r#"{{"id":"bulk-{i}","peer":"{b_peer}","sender":"me","kind":"text","tsMs":{},"text":"bulk-{i}","media":null,"status":"failed","replyTo":null}}"#,
        1700000000000i64 + i as i64
    )
}

#[tokio::test]
async fn large_backlog_converges_over_repeated_flushes() {
    // 批量上限语义：单次 flush 只处理一批（FLUSH_BATCH_CAP），剩余靠后续触发收敛，
    // 两端最终一致且不死锁。
    let dir = std::env::temp_dir().join(format!("p2p-chat-cap40-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let a = spawn_at("cap40-a", &dir).await;
    let b = spawn("cap40-b").await;
    add_each_other(&a, &b).await;
    let b_peer = peer_str(&b.node);
    let a_peer = peer_str(&a.node);

    let lines: Vec<String> = (0..40).map(|i| failed_line(i, &b_peer)).collect();
    let outbox_path = a.dir.join("chat/outbox").join(format!("{b_peer}.jsonl"));
    std::fs::create_dir_all(outbox_path.parent().expect("outbox parent")).expect("mkdir");
    std::fs::write(&outbox_path, lines.join("\n") + "\n").expect("write backlog");
    let messages_path = a.dir.join("chat/messages").join(format!("{b_peer}.jsonl"));
    std::fs::create_dir_all(messages_path.parent().expect("messages parent")).expect("mkdir");
    std::fs::write(&messages_path, lines.join("\n") + "\n").expect("write messages");

    a.node.shutdown();
    let a2 = spawn_at("cap40-a2", &dir).await;
    a2.chat
        .friend_add(&b_peer, "b", b.node.listen_addrs(), None)
        .expect("re-add");

    let _ = a2
        .chat
        .drain_peer(&b_peer, std::time::Duration::from_secs(10))
        .await;
    let _ = a2
        .chat
        .drain_peer(&b_peer, std::time::Duration::from_secs(10))
        .await;
    wait_until("backlog fully delivered", || {
        b.chat
            .history(&a_peer, None, 100)
            .map(|m| m.len() >= 40)
            .unwrap_or(false)
    })
    .await;
    wait_until("a outbox empty", || outbox_lines(&a2, &b_peer) == 0).await;
    cleanup(&a2);
    cleanup(&b);
    let _ = std::fs::remove_dir_all(&dir);
}
