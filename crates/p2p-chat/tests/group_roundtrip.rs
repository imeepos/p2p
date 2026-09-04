//! 双节点群消息 roundtrip（design §6）：文本/附件实时送达、roster 事件、
//! 离线成员上线 flush（roster 先于消息入队，最终一致）。

mod common;

use common::{
    add_each_other, cleanup, parse_peer, peer_str, spawn, spawn_at, wait_event, wait_until,
};
use p2p_chat::{ChatKind, ChatMediaInput, ChatStatus, GroupEvent, GroupState};

/// (a) A 建群 [B] → B 收 roster 状态事件；A 发文本 → B 收消息事件 + 历史；
/// A 侧 acks 推进至 delivered；replyTo 透传。
#[tokio::test]
async fn group_text_roundtrip_two_nodes() {
    let a = spawn("gr-a").await;
    let b = spawn("gr-b").await;
    add_each_other(&a, &b).await;
    let peer_b = peer_str(&b.node);

    let mut ev_b = b.chat.group_events();
    let g = a
        .chat
        .group
        .group_create("双人群", std::slice::from_ref(&peer_b))
        .await
        .expect("create");
    let joined = wait_event(
        &mut ev_b,
        |e| matches!(e, GroupEvent::State { .. }),
        "B 收 roster",
    )
    .await;
    let GroupEvent::State { group } = joined else {
        panic!("unreachable")
    };
    assert_eq!(group.group_id, g.group_id);
    assert_eq!(group.owner, peer_str(&a.node), "B 视角 owner = A");
    assert!(group.members.contains(&peer_b), "roster members 含本机 B");

    let report = a
        .chat
        .group
        .group_send(
            &g.group_id,
            ChatKind::Text,
            Some("大家好".into()),
            None,
            None,
        )
        .await
        .expect("send text");
    assert_eq!(report.recipients, 1);
    assert_eq!(report.acked, 1, "在线成员实时确认");
    assert!(report.delivered);
    assert_eq!(report.message.status, ChatStatus::Delivered);

    let msg_ev = wait_event(
        &mut ev_b,
        |e| matches!(e, GroupEvent::Message { .. }),
        "B 收消息",
    )
    .await;
    let GroupEvent::Message { group_id, message } = msg_ev else {
        panic!("unreachable")
    };
    assert_eq!(group_id, g.group_id);
    assert_eq!(message.sender_id, peer_str(&a.node), "sender_id = 作者 A");
    assert_eq!(message.text.as_deref(), Some("大家好"));
    assert_eq!(message.kind, ChatKind::Text);
    assert_eq!(message.status, ChatStatus::Delivered);

    let b_history = b
        .chat
        .group
        .group_history(&g.group_id, None, 10)
        .expect("b history");
    assert_eq!(b_history.len(), 1);
    assert!(b_history[0].acks.is_empty(), "收到的消息 acks 恒空");

    // 带引用的第二条
    let reply_id = report.message.id.clone();
    let report2 = a
        .chat
        .group
        .group_send(
            &g.group_id,
            ChatKind::Text,
            Some("引用".into()),
            None,
            Some(reply_id.clone()),
        )
        .await
        .expect("send reply");
    assert_eq!(report2.message.reply_to.as_deref(), Some(reply_id.as_str()));
    wait_until("B 历史 2 条", || {
        b.chat
            .group
            .group_history(&g.group_id, None, 10)
            .is_ok_and(|h| h.len() == 2)
    })
    .await;

    common::cleanup(&a);
    cleanup(&b);
}

/// (b) 图片附件 roundtrip：发端落盘 media/<groupId>/，收端同目录落盘可读、内容一致。
#[tokio::test]
async fn group_media_roundtrip() {
    let a = spawn("gm-a").await;
    let b = spawn("gm-b").await;
    add_each_other(&a, &b).await;
    let peer_b = peer_str(&b.node);
    let mut ev_b = b.chat.group_events();
    let g = a
        .chat
        .group
        .group_create("附件群", std::slice::from_ref(&peer_b))
        .await
        .expect("create");
    let _ = wait_event(
        &mut ev_b,
        |e| matches!(e, GroupEvent::State { .. }),
        "B 收 roster",
    )
    .await;

    let png = vec![0x89u8, b'P', b'N', b'G', 1, 2, 3, 4, 5, 6, 7, 8];
    let report = a
        .chat
        .group
        .group_send(
            &g.group_id,
            ChatKind::Image,
            None,
            Some(ChatMediaInput {
                name: "shot.png".into(),
                mime: "image/png".into(),
                data: png.clone(),
            }),
            None,
        )
        .await
        .expect("send image");
    assert!(report.delivered, "附件实时送达");

    let sender_path = report
        .message
        .media
        .as_ref()
        .expect("sender media")
        .path
        .clone()
        .expect("path");
    assert!(
        sender_path.contains(&format!("media/{}", g.group_id)),
        "发端落盘 media/<groupId>/: {sender_path}"
    );

    let GroupEvent::Message { message, .. } = wait_event(
        &mut ev_b,
        |e| matches!(e, GroupEvent::Message { .. }),
        "B 收附件消息",
    )
    .await
    else {
        panic!("unreachable");
    };
    let meta = b
        .chat
        .group
        .group_media_file(&g.group_id, &message.id)
        .expect("b media file");
    assert_eq!(meta.mime, "image/png");
    let bytes = std::fs::read(meta.path.expect("b path")).expect("read media");
    assert_eq!(bytes, png, "收端字节与发端一致");
    cleanup(&a);
    cleanup(&b);
}

/// (c) 离线成员上线 flush：C 离线时建群+发消息（goutbox pending）→ C 重启上线
/// → roster 先补投（否则消息 unknown_group）→ 消息随后送达。
#[tokio::test]
async fn offline_member_flush_after_rejoin() {
    let a = spawn("gf-a").await;
    let c_dir = std::env::temp_dir().join(format!("p2p-chat-gf-c-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&c_dir);
    let c = spawn_at("gf-c", &c_dir).await;
    let peer_a = peer_str(&a.node);
    let peer_c = peer_str(&c.node);
    a.chat
        .friend_add(&peer_c, "c", c.node.listen_addrs(), None)
        .expect("a add c");
    c.chat
        .friend_add(&peer_a, "a", a.node.listen_addrs(), None)
        .expect("c add a");

    c.node.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let g = a
        .chat
        .group
        .group_create("离线群", std::slice::from_ref(&peer_c))
        .await
        .expect("create offline");
    let report = a
        .chat
        .group
        .group_send(
            &g.group_id,
            ChatKind::Text,
            Some("补投消息".into()),
            None,
            None,
        )
        .await
        .expect("send offline");
    assert!(!report.delivered, "离线成员不得实时送达");
    assert_eq!(report.acked, 0);

    // C 重启（同身份），A 刷新 C 地址后 C 拨 A → A 侧 PeerConnected 触发双队列 flush
    let c2 = spawn_at("gf-c", &c_dir).await;
    assert_eq!(peer_str(&c2.node), peer_c, "重启身份不变");
    a.chat
        .friend_add(&peer_c, "c", c2.node.listen_addrs(), None)
        .expect("a 刷新 c 地址");
    let mut ev_c2 = c2.chat.group_events();
    c2.node
        .connect(parse_peer(&peer_a))
        .await
        .expect("c2 dial a");

    let joined = wait_event(
        &mut ev_c2,
        |e| matches!(e, GroupEvent::State { .. }),
        "C2 补投 roster",
    )
    .await;
    let GroupEvent::State { group } = joined else {
        panic!("unreachable")
    };
    assert_eq!(group.group_id, g.group_id);
    assert_eq!(group.state, GroupState::Active);

    let msg = wait_event(
        &mut ev_c2,
        |e| matches!(e, GroupEvent::Message { .. }),
        "C2 补投消息",
    )
    .await;
    let GroupEvent::Message { message, .. } = msg else {
        panic!("unreachable")
    };
    assert_eq!(message.text.as_deref(), Some("补投消息"));

    wait_until("A 侧 acks 到齐 delivered", || {
        a.chat
            .group
            .group_history(&g.group_id, None, 10)
            .is_ok_and(|h| h.iter().all(|m| m.acks.len() == 1))
    })
    .await;
    cleanup(&a);
    cleanup(&c2);
}
