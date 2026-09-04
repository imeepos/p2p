//! 回归（跨机演练 D1/D4）：背靠背发送与 outbox 毒化场景下新发送必须可达。
//! 演练实锤：快速连发第二条必 failed；outbox failed 条目堆积后节点外拨全灭。

use p2p_chat::{Chat, ChatKind, ChatStatus};

mod common;
use common::{add_each_other, cleanup, outbox_lines, peer_str, spawn, spawn_at, wait_until};

fn delivered_count(chat: &Chat, peer: &str) -> usize {
    chat.history(peer, None, 100)
        .expect("history reads")
        .iter()
        .filter(|m| m.status == ChatStatus::Delivered)
        .count()
}

fn failed_line(id: &str, ts: i64, b_peer: &str) -> String {
    format!(
        r#"{{"id":"{id}","peer":"{b_peer}","sender":"me","kind":"text","tsMs":{ts},"text":"{id}","media":null,"status":"failed","replyTo":null}}"#
    )
}

#[tokio::test]
async fn back_to_back_sends_all_deliver_without_failed_gap() {
    let a = spawn("rapid-a").await;
    let b = spawn("rapid-b").await;
    add_each_other(&a, &b).await;
    let b_peer = peer_str(&b.node);

    // 背靠背三条：第二条命中 D1 竞态面（演练中 3/3 复现 failed）。
    for i in 1..=3 {
        let report = a
            .chat
            .send(
                &b_peer,
                ChatKind::Text,
                Some(format!("rapid-{i}")),
                None,
                None,
            )
            .await
            .expect("send is accepted");
        assert!(
            report.delivered,
            "rapid send {i} must deliver, got status={:?}",
            report.message.status
        );
    }

    wait_until("b receives all three", || {
        delivered_count(&b.chat, &peer_str(&a.node)) >= 3
    })
    .await;
    cleanup(&a);
    cleanup(&b);
}

#[tokio::test]
async fn fabricated_failed_outbox_entries_do_not_poison_new_sends() {
    let dir = std::env::temp_dir().join(format!("p2p-chat-poison-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let a = spawn_at("poison-a", &dir).await;
    let b = spawn("poison-b").await;
    add_each_other(&a, &b).await;
    let b_peer = peer_str(&b.node);

    // 直接落 5 条 failed outbox 条目（演练 D4 的毒化形态）+ 1 条必然失败的媒体条目。
    let outbox_path = a.dir.join("chat/outbox").join(format!("{b_peer}.jsonl"));
    std::fs::create_dir_all(outbox_path.parent().expect("outbox parent")).expect("mkdir outbox");
    let mut lines: Vec<String> = (0..5)
        .map(|i| failed_line(&format!("poison-{i}"), 1700000000000i64 + i, &b_peer))
        .collect();
    lines.push(format!(
        r#"{{"id":"poison-media","peer":"{b_peer}","sender":"me","kind":"file","tsMs":1700000000100,"text":null,"media":{{"name":"gone.bin","mime":"application/octet-stream","size":10,"path":"/nonexistent/gone.bin"}},"status":"failed","replyTo":null}}"#
    ));
    std::fs::write(&outbox_path, lines.join("\n") + "\n").expect("write poisoned outbox");
    // 真实流里消息记录在发送时已落 messages：同步补齐，供死信后保留记录断言。
    let messages_path = a.dir.join("chat/messages").join(format!("{b_peer}.jsonl"));
    std::fs::create_dir_all(messages_path.parent().expect("messages parent"))
        .expect("mkdir messages");
    let msg_rows: Vec<String> = (0..5)
        .map(|i| failed_line(&format!("poison-{i}"), 1700000000000i64 + i, &b_peer))
        .chain(std::iter::once(lines.last().expect("media line").clone()))
        .collect();
    std::fs::write(&messages_path, msg_rows.join("\n") + "\n").expect("write poisoned messages");

    // 重新拉起 A（Store::new 装载毒化 outbox），对端在线。
    a.node.shutdown();
    let a2 = spawn_at("poison-a2", &dir).await;
    a2.chat
        .friend_add_direct(&b_peer, "b", b.node.listen_addrs(), None)
        .expect("re-add friend");

    // 新发送必须不被毒化条目阻塞。
    let report = a2
        .chat
        .send(
            &b_peer,
            ChatKind::Text,
            Some("after-poison".into()),
            None,
            None,
        )
        .await
        .expect("send accepted");
    assert!(
        report.delivered,
        "new send must deliver despite poisoned outbox, got {:?}",
        report.message.status
    );

    // 首轮 flush：可达条目全部送达（含曾 failed 的）；必然失败条目留在队内。
    wait_until("b sees poison texts", || {
        delivered_count(&b.chat, &peer_str(&a2.node)) >= 5
    })
    .await;

    // 二次连接触发 flush：已给过机会的必然失败条目死信出队（P3 纪律）。
    let bpid = common::parse_peer(&b_peer);
    a2.node.disconnect(&bpid);
    a2.node.connect(bpid).await.expect("reconnect b");
    wait_until("outbox fully drained by dead-letter", || {
        outbox_lines(&a2, &b_peer) == 0
    })
    .await;
    let media_row = a2
        .chat
        .history(&b_peer, None, 100)
        .expect("history reads")
        .into_iter()
        .find(|m| m.id == "poison-media")
        .expect("dead-lettered entry keeps its message record");
    assert_eq!(
        media_row.status,
        ChatStatus::Failed,
        "dead letter stays failed"
    );

    cleanup(&a2);
    cleanup(&b);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn drain_peer_delivers_offline_queue_after_peer_returns() {
    // 演练 2.8 CLI 化：离线积压 1 条，对端回归后一次 drain 全部补投（D5）。
    let a = spawn("drain-a").await;
    let b = spawn("drain-b").await;
    add_each_other(&a, &b).await;
    let a_peer = peer_str(&a.node);
    let b_peer = peer_str(&b.node);

    // B 下线，A 发送 → 保持 pending。
    b.node.shutdown();
    let report = a
        .chat
        .send(
            &b_peer,
            ChatKind::Text,
            Some("offline queue me".into()),
            None,
            None,
        )
        .await
        .expect("send accepted");
    assert!(!report.delivered, "offline send must stay pending");

    // B 回归（新端口），A 刷新地址簿后 drain。
    let b2 = spawn_at("drain-b2", &b.dir).await;
    b2.chat
        .friend_add_direct(&a_peer, "a", a.node.listen_addrs(), None)
        .expect("b2 re-add a");
    a.chat
        .friend_add_direct(&b_peer, "b", b2.node.listen_addrs(), None)
        .expect("a refresh b addrs");

    let drained = a
        .chat
        .drain_peer(&b_peer, std::time::Duration::from_secs(10))
        .await
        .expect("drain runs");
    assert_eq!(drained, 1, "exactly the offline entry is drained");

    wait_until("b2 history has offline message", || {
        b2.chat
            .history(&a_peer, None, 10)
            .map(|m| m.len() == 1)
            .unwrap_or(false)
    })
    .await;
    let row = a.chat.history(&b_peer, None, 10).expect("a history")[0].clone();
    assert_eq!(
        row.status,
        ChatStatus::Delivered,
        "record flips to delivered"
    );
    cleanup(&a);
    cleanup(&b2);
}
