//! itest (c/d)：对端离线 → pending → 上线 flush → delivered；
//! 好友簿 add/remove/list 原子性与非法 peerId/地址/昵称拒绝。

mod common;

use std::time::Duration;

use common::{
    add_each_other, cleanup, node_events, outbox_lines, parse_peer, peer_str, spawn, spawn_at,
    wait_event, wait_until,
};
use p2p::NodeEvent;
use p2p_chat::{ChatEvent, ChatKind, ChatStatus};

/// (c) 离线投递：B 离线时 A 发送 → pending；B 上线后 A 侧收到 PeerConnected
/// → outbox flush 重发 → delivered，B 侧收到 chat_message。
#[tokio::test]
async fn offline_pending_then_flush_on_peer_connected() {
    let a = spawn("of-a").await;
    let b = spawn("of-b").await;
    add_each_other(&a, &b).await;
    let peer_a = peer_str(&a.node);
    let peer_b = peer_str(&b.node);

    // B 下线；A 侧订阅事件以便断言 PeerConnected
    let mut ev_a = node_events(&a.node);
    b.node.shutdown();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // B 离线发送：connect 失败 → 保持 pending，不标记 failed
    let report = a
        .chat
        .send(&peer_b, ChatKind::Text, Some("离线消息".into()), None, None)
        .await
        .expect("send while offline");
    assert!(!report.delivered, "离线发送不能实时送达");
    assert_eq!(report.message.status, ChatStatus::Pending);
    assert_eq!(outbox_lines(&a, &peer_b), 1, "outbox 留 1 条待发");

    // B 重启上线（同 data_dir = 同身份）。端口 0 动态分配后重启端口必变，
    // A 地址簿刷新 B2 当前监听地址（friend_add upsert），flush 才可拨。
    let b2 = spawn_at("of-b", &b.dir).await;
    assert_eq!(peer_str(&b2.node), peer_b, "重启身份不变");
    let mut ev_b2 = b2.chat.events();
    a.chat
        .friend_add(&peer_b, "b", b2.node.listen_addrs(), None)
        .expect("a 刷新 b 重启后地址");

    // B 主动拨 A（模拟应用重连）：A 侧触发 PeerConnected(B) → flush
    b2.node
        .connect(parse_peer(&peer_a))
        .await
        .expect("b2 dial a");
    wait_event(
        &mut ev_a,
        |ev| matches!(ev, NodeEvent::PeerConnected { peer } if peer.to_string() == peer_b),
        "A 侧 PeerConnected(B)",
    )
    .await;

    // A 的 outbox 任务 flush：消息 delivered，outbox 清空
    wait_until("A 侧 delivered 且 outbox 清空", || {
        let delivered = a
            .chat
            .history(&peer_b, None, 10)
            .map(|h| h.iter().any(|m| m.status == ChatStatus::Delivered))
            .unwrap_or(false);
        delivered && outbox_lines(&a, &peer_b) == 0
    })
    .await;

    // B 侧收到 chat_message 事件 + 落盘
    wait_event(
        &mut ev_b2,
        |ev| matches!(ev, ChatEvent::ChatMessage { .. }),
        "B 侧 chat_message",
    )
    .await;
    let b_msgs = b2.chat.history(&peer_a, None, 10).expect("b2 history");
    assert_eq!(b_msgs.len(), 1);
    assert_eq!(b_msgs[0].text.as_deref(), Some("离线消息"));
    assert_eq!(b_msgs[0].status, ChatStatus::Delivered);

    cleanup(&a);
    cleanup(&b);
}

/// (d) 好友簿：非法 peerId/自加/超长昵称/非法地址拒绝；add→upsert→remove
/// 幂等；yrs 更新日志逐变更一行落盘（含未命中 remove 零追加）。
#[tokio::test]
async fn friends_add_remove_list_and_invalid_reject() {
    let a = spawn("fr-a").await;
    let b = spawn("fr-b").await;
    let peer_b = peer_str(&b.node);
    let own = peer_str(&a.node);

    // 非法 peerId 拒绝（非 base58）
    let err = a
        .chat
        .friend_add("not-a-peer!!", "x", vec![], None)
        .unwrap_err();
    assert!(err.to_string().contains("base58"), "err: {err}");
    // 自加拒绝
    let err = a.chat.friend_add(&own, "self", vec![], None).unwrap_err();
    assert!(err.to_string().contains("自己"), "err: {err}");
    // 超长昵称拒绝
    let err = a
        .chat
        .friend_add(&peer_b, &"n".repeat(65), vec![], None)
        .unwrap_err();
    assert!(err.to_string().contains("昵称"), "err: {err}");
    // 非法地址拒绝
    let err = a
        .chat
        .friend_add(&peer_b, "b", vec!["bad-addr".into()], None)
        .unwrap_err();
    assert!(err.to_string().contains("地址"), "err: {err}");

    // add
    let f = a
        .chat
        .friend_add(&peer_b, "小 b", b.node.listen_addrs(), Some("同事".into()))
        .expect("add friend");
    assert_eq!(f.nickname, "小 b");
    assert_eq!(f.peer_id, peer_b);
    assert!(a.chat.friends_list().is_ok_and(|l| l.len() == 1));

    // 重复 add = upsert（同 peerId 覆盖）
    a.chat
        .friend_add(&peer_b, "小 b2", vec![], None)
        .expect("upsert");
    let list = a.chat.friends_list().expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].nickname, "小 b2");
    assert_eq!(list[0].note, None);

    // 好友簿 yrs 更新日志：格式头 + 每次实际变更一行 update（值未变零追加）
    let book_path = a.dir.join("chat/friends.json");
    let raw = std::fs::read_to_string(&book_path).expect("friends.json");
    let mut lines = raw.lines();
    let header: serde_json::Value =
        serde_json::from_str(lines.next().expect("格式头存在")).expect("格式头为 JSON");
    assert_eq!(header["p2p-friends"], "yrs-v1", "格式头: {raw}");
    assert_eq!(lines.count(), 2, "两次 add（值有变化）各追加一行 update");

    // remove 幂等：在簿 → true；不在簿 → false；不删消息历史
    assert!(a.chat.friend_remove(&peer_b).expect("remove"));
    assert!(!a.chat.friend_remove(&peer_b).expect("remove again"));
    assert!(a.chat.friends_list().is_ok_and(|l| l.is_empty()));
    // 好友簿空时 friends.json 仍存在：头行 + 3 行 update（未命中 remove 不追加）
    let raw = std::fs::read_to_string(&book_path).expect("friends.json");
    assert_eq!(raw.lines().count(), 4, "add+upsert+remove 各一行: {raw}");

    cleanup(&a);
    cleanup(&b);
}
