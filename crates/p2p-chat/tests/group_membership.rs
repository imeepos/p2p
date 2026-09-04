//! 成员操作语义（design §5）：invite/kick/leave/disband/rename 全链 rev 收敛；
//! owner-only 显式拒绝；被移/退群者禁发；owner 不能退群。

mod common;

use common::{add_each_other, cleanup, peer_str, spawn, wait_until};
use p2p_chat::{ChatKind, GroupState};

/// 三节点互相为 A 的好友；A 建 [B, C]，B/C 在线收 roster。
async fn setup(tag: &str) -> (common::TestNode, common::TestNode, common::TestNode, String) {
    let a = spawn(&format!("{tag}-a")).await;
    let b = spawn(&format!("{tag}-b")).await;
    let c = spawn(&format!("{tag}-c")).await;
    add_each_other(&a, &b).await;
    add_each_other(&a, &c).await;
    let peer_b = peer_str(&b.node);
    let peer_c = peer_str(&c.node);
    let g = a
        .chat
        .group
        .group_create("语义群", &[peer_b.clone(), peer_c.clone()])
        .await
        .expect("create");
    wait_until("B/C 收 roster", || {
        b.chat
            .group
            .group_list()
            .iter()
            .any(|x| x.group_id == g.group_id)
            && c.chat
                .group
                .group_list()
                .iter()
                .any(|x| x.group_id == g.group_id)
    })
    .await;
    (a, b, c, g.group_id)
}

#[tokio::test]
async fn invite_kick_leave_disband_semantics() {
    let (a, b, c, group_id) = setup("mem").await;
    let peer_b = peer_str(&b.node);
    let peer_c = peer_str(&c.node);
    let peer_d = p2p_identity::Keypair::generate().peer_id().to_string();
    a.chat
        .friend_add(&peer_d, "d", vec![], None)
        .expect("a add d");

    // 邀请 D（离线，goutbox 补投）：A rev+1、成员 4
    let g = a
        .chat
        .group
        .group_invite(&group_id, std::slice::from_ref(&peer_d))
        .await
        .expect("invite");
    assert_eq!(g.rev, 1);
    assert_eq!(g.members.len(), 4);

    // 踢 B：A rev+2 且 B 不在名单；B 端被踢后禁发
    let g = a
        .chat
        .group
        .group_kick(&group_id, &peer_b)
        .await
        .expect("kick");
    assert_eq!(g.rev, 2);
    assert!(!g.members.contains(&peer_b));
    wait_until("B 端 state=kicked", || {
        b.chat
            .group
            .group_list()
            .iter()
            .any(|x| x.group_id == group_id && x.state == GroupState::Kicked)
    })
    .await;
    let err = b
        .chat
        .group
        .group_send(&group_id, ChatKind::Text, Some("还想说".into()), None, None)
        .await
        .expect_err("被踢者禁发");
    assert!(err.to_string().contains("禁止发送"), "err: {err}");

    // C 退群：本端 left、历史保留；A rev+3 且名单移除
    let g = c.chat.group.group_leave(&group_id).await.expect("leave");
    assert_eq!(g.state, GroupState::Left);
    wait_until("A 端 rev=3 且无 C", || {
        a.chat
            .group
            .group_list()
            .iter()
            .any(|x| x.group_id == group_id && x.rev == 3 && !x.members.contains(&peer_c))
    })
    .await;

    // owner 不能退群；非 owner 的 owner 操作拒绝
    let err = a
        .chat
        .group
        .group_leave(&group_id)
        .await
        .expect_err("owner 退群必须拒绝");
    assert!(err.to_string().contains("解散"), "err: {err}");
    let err = b
        .chat
        .group
        .group_rename(&group_id, "冒名")
        .await
        .expect_err("非 owner 改名必须拒绝");
    assert!(err.to_string().contains("群主"), "err: {err}");

    // 改名：rev+1 推 roster
    let g = a
        .chat
        .group
        .group_rename(&group_id, "改名群")
        .await
        .expect("rename");
    assert_eq!(g.name, "改名群");
    assert_eq!(g.rev, 4);

    // 解散 g1：B 已被踢、C 已退群 → 无在册成员可通知，A 本端 state=disbanded
    let g = a
        .chat
        .group
        .group_disband(&group_id)
        .await
        .expect("disband");
    assert_eq!(g.state, GroupState::Disbanded);

    // 解散通知链：新群 A+[C] → C 收 G_KICK(disbanded) → state 置位禁发
    let g2 = a
        .chat
        .group
        .group_create("解散群", std::slice::from_ref(&peer_c))
        .await
        .expect("create g2");
    wait_until("C 收 g2 roster", || {
        c.chat
            .group
            .group_list()
            .iter()
            .any(|x| x.group_id == g2.group_id)
    })
    .await;
    let g2 = a
        .chat
        .group
        .group_disband(&g2.group_id)
        .await
        .expect("disband g2");
    assert_eq!(g2.state, GroupState::Disbanded);
    wait_until("C 端 g2 state=disbanded", || {
        c.chat
            .group
            .group_list()
            .iter()
            .any(|x| x.group_id == g2.group_id && x.state == GroupState::Disbanded)
    })
    .await;
    let err = c
        .chat
        .group
        .group_send(
            &g2.group_id,
            ChatKind::Text,
            Some("最后一次".into()),
            None,
            None,
        )
        .await
        .expect_err("解散后禁发");
    assert!(err.to_string().contains("禁止发送"), "err: {err}");
    cleanup(&a);
    cleanup(&b);
    cleanup(&c);
}

/// 历史保留：被踢/退群/解散不删数据，group_history 仍可分页读。
#[tokio::test]
async fn history_preserved_after_state_changes() {
    let (a, b, c, group_id) = setup("hist").await;
    let peer_c = peer_str(&c.node);
    a.chat
        .group
        .group_send(&group_id, ChatKind::Text, Some("留痕".into()), None, None)
        .await
        .expect("send");
    wait_until("C 收到消息", || {
        c.chat
            .group
            .group_history(&group_id, None, 10)
            .is_ok_and(|h| !h.is_empty())
    })
    .await;
    c.chat.group.group_leave(&group_id).await.expect("leave");
    let hist = c
        .chat
        .group
        .group_history(&group_id, None, 10)
        .expect("c history after leave");
    assert!(!hist.is_empty(), "退群不删历史");
    assert!(hist.iter().any(|m| m.text.as_deref() == Some("留痕")));
    let _ = peer_c;
    cleanup(&a);
    cleanup(&b);
    cleanup(&c);
}
