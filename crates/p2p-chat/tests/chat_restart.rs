//! itest (e)：重启后 outbox 恢复——A 发送时 B 离线（pending），A 重启
//! （同 data_dir 身份不变）重新加载 outbox；B 上线后触发 flush 重发 → delivered。

mod common;

use std::time::Duration;

use common::{
    add_each_other, cleanup, node_events, outbox_lines, parse_peer, peer_str, spawn, spawn_at,
    wait_event, wait_until,
};
use p2p::NodeEvent;
use p2p_chat::{ChatEvent, ChatKind, ChatStatus};

#[tokio::test]
async fn restart_recovers_outbox_and_flushes() {
    // A/B 首次启动：端口 0 内核动态分配（不再依赖固定端口）
    let a = spawn("rs-a").await;
    let b = spawn("rs-b").await;
    add_each_other(&a, &b).await;
    let peer_a = peer_str(&a.node);
    let peer_b = peer_str(&b.node);

    // B 离线
    b.node.shutdown();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // A 发送 → pending，outbox 落盘 1 条
    let report = a
        .chat
        .send(
            &peer_b,
            ChatKind::Text,
            Some("重启后送达".into()),
            None,
            None,
        )
        .await
        .expect("send while offline");
    assert!(!report.delivered);
    assert_eq!(report.message.status, ChatStatus::Pending);
    assert_eq!(outbox_lines(&a, &peer_b), 1, "outbox 落盘 1 条");

    // A 重启：同 data_dir（保留 key.seed → 身份不变）；端口 0 动态分配
    let a_dir = a.dir.clone();
    a.node.shutdown();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let a2 = spawn_at("rs-a", &a_dir).await;
    assert_eq!(peer_str(&a2.node), peer_a, "重启身份不变");
    let mut ev_a2 = node_events(&a2.node);

    // B 重启上线（同 data_dir → 身份不变）
    let b2 = spawn_at("rs-b", &b.dir).await;
    assert_eq!(peer_str(&b2.node), peer_b);
    let mut ev_b2 = b2.chat.events();

    // 双方地址簿互相刷新为重启后的真实监听地址（端口已变，旧地址不可拨）
    a2.chat
        .friend_add_direct(&peer_b, "b", b2.node.listen_addrs(), None)
        .expect("a2 刷新 b2 地址");
    b2.chat
        .friend_add_direct(&peer_a, "a", a2.node.listen_addrs(), None)
        .expect("b2 刷新 a2 地址");

    // 触发连接：B2 拨 A2
    b2.node
        .connect(parse_peer(&peer_a))
        .await
        .expect("b2 dial a2");
    wait_event(
        &mut ev_a2,
        |ev| matches!(ev, NodeEvent::PeerConnected { peer } if peer.to_string() == peer_b),
        "A2 侧 PeerConnected(B)",
    )
    .await;

    // A2 outbox flush：重发 pending → delivered，outbox 清空
    wait_until("A2 delivered 且 outbox 清空", || {
        let delivered = a2
            .chat
            .history(&peer_b, None, 10)
            .map(|h| h.iter().any(|m| m.status == ChatStatus::Delivered))
            .unwrap_or(false);
        delivered && outbox_lines(&a2, &peer_b) == 0
    })
    .await;

    // B2 收到重启后重发的消息
    wait_event(
        &mut ev_b2,
        |ev| matches!(ev, ChatEvent::ChatMessage { .. }),
        "B2 侧 chat_message",
    )
    .await;
    let b_msgs = b2.chat.history(&peer_a, None, 10).expect("b2 history");
    assert_eq!(b_msgs.len(), 1);
    assert_eq!(b_msgs[0].text.as_deref(), Some("重启后送达"));
    assert_eq!(b_msgs[0].status, ChatStatus::Delivered);

    cleanup(&b);
}
