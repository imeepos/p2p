//! itest（a/b）：文本送达 + ACK + 双端落盘；附件 64MiB 分片边界与超限拒绝。
//! 双节点真实回环（非 mock）：经 Node 拨号 → 开流 → /im/chat/1 帧序 → ACK。

mod common;

use common::{add_each_other, cleanup, peer_str, spawn, wait_until};
use p2p_chat::{ChatKind, ChatMediaInput, ChatStatus, Sender, MAX_MESSAGE_SIZE};

/// (a) 文本送达：实时 delivered、ACK 语义、双端 messages/<peer>.jsonl 落盘。
#[tokio::test]
async fn text_delivery_ack_and_both_persist() {
    let a = spawn("rt-a").await;
    let b = spawn("rt-b").await;
    add_each_other(&a, &b).await;
    let peer_a = peer_str(&a.node);
    let peer_b = peer_str(&b.node);

    let report = a
        .chat
        .send(
            &peer_b,
            ChatKind::Text,
            Some("你好，世界".into()),
            None,
            None,
        )
        .await
        .expect("send text");
    assert!(report.delivered, "实时送达必须 true");
    assert_eq!(report.message.status, ChatStatus::Delivered);
    assert_eq!(report.message.sender, Sender::Me);
    assert!(!report.message.id.is_empty(), "发端生成 UUID");

    // 对端落盘：messages/<peer_a>.jsonl 一行，sender=them
    wait_until("b 侧消息落盘", || {
        b.chat
            .history(&peer_a, None, 10)
            .map(|h| h.len() == 1)
            .unwrap_or(false)
    })
    .await;
    let b_msgs = b.chat.history(&peer_a, None, 10).expect("b history");
    assert_eq!(b_msgs[0].text.as_deref(), Some("你好，世界"));
    assert_eq!(b_msgs[0].sender, Sender::Them);
    assert_eq!(b_msgs[0].status, ChatStatus::Delivered);
    assert_eq!(b_msgs[0].id, report.message.id);

    // 本端落盘：messages/<peer_b>.jsonl，delivered
    let a_msgs = a.chat.history(&peer_b, None, 10).expect("a history");
    assert_eq!(a_msgs.len(), 1);
    assert_eq!(a_msgs[0].status, ChatStatus::Delivered);
    assert_eq!(a_msgs[0].peer, peer_b);

    // 历史分页：beforeId 严格更早（只有一条消息时游标翻页为空）
    let page = a
        .chat
        .history(&peer_b, Some(&a_msgs[0].id), 10)
        .expect("before page");
    assert!(page.is_empty(), "beforeId 游标应返回严格更早的消息");

    cleanup(&a);
    cleanup(&b);
}

/// (b) 附件：≈64MiB 分片边界送达；超限（64MiB+1）发送前拒绝（可读中文 Err）。
#[tokio::test]
async fn attachment_chunk_boundary_and_oversize_reject() {
    let a = spawn("at-a").await;
    let b = spawn("at-b").await;
    add_each_other(&a, &b).await;
    let peer_a = peer_str(&a.node);
    let peer_b = peer_str(&b.node);

    // 超限拒绝：不产生发送，错误可读中文且含上限
    let oversize = vec![7u8; MAX_MESSAGE_SIZE as usize + 1];
    let err = a
        .chat
        .send(
            &peer_b,
            ChatKind::File,
            None,
            Some(ChatMediaInput {
                name: "too-big.bin".into(),
                mime: "application/octet-stream".into(),
                data: oversize,
            }),
            None,
        )
        .await
        .expect_err("超限必须 Err");
    assert!(
        err.to_string().contains("64MiB"),
        "超限错误应含上限说明：{err}"
    );
    assert!(
        b.chat
            .history(&peer_a, None, 10)
            .is_ok_and(|h| h.is_empty()),
        "超限消息不得落盘"
    );

    // ≈64MiB 恰好边界：分片传输 + 对端完整落盘
    let payload: Vec<u8> = (0..MAX_MESSAGE_SIZE as usize)
        .map(|i| (i % 251) as u8)
        .collect();
    let report = a
        .chat
        .send(
            &peer_b,
            ChatKind::File,
            None,
            Some(ChatMediaInput {
                name: "chunk-boundary.bin".into(),
                mime: "application/octet-stream".into(),
                data: payload.clone(),
            }),
            None,
        )
        .await
        .expect("send 64MiB file");
    assert!(report.delivered);

    wait_until("b 侧附件消息落盘", || {
        b.chat
            .history(&peer_a, None, 10)
            .map(|h| h.iter().any(|m| m.kind == ChatKind::File))
            .unwrap_or(false)
    })
    .await;
    let b_msgs = b.chat.history(&peer_a, None, 10).expect("b history");
    let file_msg = b_msgs
        .iter()
        .find(|m| m.kind == ChatKind::File)
        .expect("file message");
    let meta = b
        .chat
        .media_file(&peer_a, &file_msg.id)
        .expect("media meta");
    let saved_path = meta.path.clone().expect("saved path");
    let saved = std::fs::read(&saved_path).expect("read saved media");
    assert_eq!(saved.len() as u64, MAX_MESSAGE_SIZE, "对端媒体完整");
    assert_eq!(saved[12345], payload[12345], "媒体内容一致");
    assert_eq!(meta.size, MAX_MESSAGE_SIZE);
    assert_eq!(meta.name, "chunk-boundary.bin");

    cleanup(&a);
    cleanup(&b);
}
