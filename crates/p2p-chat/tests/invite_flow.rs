//! 邀请制加好友端到端流（真实双节点）：邀请 → 同意 → 双向互为好友；拒绝/幂等/自愈。
//! 断言允许 expect/panic（tests 目录豁免 panic-hygiene）；行数受 300 红线约束。

mod common;

use common::{add_each_other, cleanup, peer_str, spawn, wait_event, wait_until, TestNode};
use p2p_chat::{ChatError, ChatEvent, FriendInvite, InviteDirection, InviteState};

async fn invite_of(node: &TestNode, peer: &str, direction: InviteDirection) -> Option<FriendInvite> {
    node.chat
        .invites_list()
        .expect("invites_list")
        .into_iter()
        .find(|i| i.peer_id == peer && i.direction == direction)
}

fn invite_event(peer: &str, state: InviteState) -> impl Fn(&ChatEvent) -> bool + '_ {
    move |ev| matches!(ev, ChatEvent::ChatInvite { peer: p, state: s } if p == peer && *s == state)
}

fn has_in_invite(node: &TestNode, peer: &str) -> bool {
    node.chat
        .invites_list()
        .expect("invites_list")
        .iter()
        .any(|i| i.peer_id == peer && i.direction == InviteDirection::In)
}

fn has_friend(node: &TestNode, peer: &str) -> bool {
    node.chat
        .friends_list()
        .expect("friends_list")
        .iter()
        .any(|f| f.peer_id == peer)
}

#[tokio::test]
async fn invite_accept_builds_mutual_friendship() {
    let a = spawn("inv-acc-a").await;
    let b = spawn("inv-acc-b").await;
    let (a_peer, b_peer) = (peer_str(&a.node), peer_str(&b.node));
    let mut b_events = b.chat.events();

    // 邀请：A 侧仅登记 out 邀请，好友簿不变（同意前不生效）
    let report = a
        .chat
        .friend_invite(&b_peer, "小 b", b.node.listen_addrs(), None)
        .await
        .expect("friend_invite");
    assert!(report.delivered, "对端地址已登记，本用例应送达");
    assert!(!has_friend(&a, &b_peer), "同意前 A 不得有好友条目");
    let in_invite = invite_of(&b, &a_peer, InviteDirection::In)
        .await
        .expect("B 应持有来邀");
    assert_eq!(in_invite.nickname, "小 b");
    assert!(!in_invite.addrs.is_empty(), "来邀应携带 A 的可回拨地址");

    // 同意：B 建好友并回投 ACCEPT；A 收 accepted 事件后建同一好友（双向互为好友）
    wait_event(&mut b_events, invite_event(&a_peer, InviteState::Incoming), "in 邀请事件").await;
    let friend = b.chat.invite_accept(&a_peer, "阿 a").await.expect("invite_accept");
    assert_eq!(friend.nickname, "阿 a");
    assert!(has_friend(&b, &a_peer), "同意后 B 侧立即建立好友");
    wait_until("A 侧经 ACCEPT 建立好友", || has_friend(&a, &b_peer)).await;
    assert!(invite_of(&a, &b_peer, InviteDirection::Out).await.is_none(), "完成后 out 邀请清除");
    assert!(invite_of(&b, &a_peer, InviteDirection::In).await.is_none(), "完成后 in 邀请清除");
    cleanup(&a);
    cleanup(&b);
}

#[tokio::test]
async fn invite_reject_notifies_inviter_and_keeps_none() {
    let a = spawn("inv-rej-a").await;
    let b = spawn("inv-rej-b").await;
    let (a_peer, b_peer) = (peer_str(&a.node), peer_str(&b.node));
    let mut a_events = a.chat.events();

    a.chat
        .friend_invite(&b_peer, "小 b", b.node.listen_addrs(), None)
        .await
        .expect("friend_invite");
    wait_until("B 侧来邀可见", || has_in_invite(&b, &a_peer)).await;
    b.chat.invite_reject(&a_peer).await.expect("invite_reject");
    assert!(invite_of(&b, &a_peer, InviteDirection::In).await.is_none());
    assert!(!has_friend(&b, &a_peer), "拒绝后 B 不得建立好友");
    wait_event(&mut a_events, invite_event(&b_peer, InviteState::Rejected), "rejected 事件").await;
    wait_until("A 侧 out 邀请被拒绝清除", || {
        a.chat
            .invites_list()
            .expect("list")
            .iter()
            .all(|i| i.peer_id != b_peer)
    })
    .await;
    assert!(invite_of(&a, &b_peer, InviteDirection::Out).await.is_none(), "被拒后 out 邀请清除");
    assert!(!has_friend(&a, &b_peer), "被拒后 A 不得有好友条目");
    cleanup(&a);
    cleanup(&b);
}

#[tokio::test]
async fn duplicate_invite_upserts_single_entry() {
    let a = spawn("inv-dup-a").await;
    let b = spawn("inv-dup-b").await;
    let b_peer = peer_str(&b.node);
    a.chat
        .friend_invite(&b_peer, "第一版", b.node.listen_addrs(), None)
        .await
        .expect("first invite");
    a.chat
        .friend_invite(&b_peer, "第二版", b.node.listen_addrs(), None)
        .await
        .expect("refresh invite");
    let outs: Vec<FriendInvite> = a
        .chat
        .invites_list()
        .expect("list")
        .into_iter()
        .filter(|i| i.direction == InviteDirection::Out)
        .collect();
    assert_eq!(outs.len(), 1, "重复邀请幂等刷新，不新增条目");
    assert_eq!(outs[0].nickname, "第二版", "以最新一次为准");
    cleanup(&a);
    cleanup(&b);
}

#[tokio::test]
async fn invite_to_existing_friend_fails_and_self_peer_fails() {
    let a = spawn("inv-exist-a").await;
    let b = spawn("inv-exist-b").await;
    add_each_other(&a, &b).await;
    let err = a
        .chat
        .friend_invite(&peer_str(&b.node), "x", b.node.listen_addrs(), None)
        .await
        .expect_err("已是好友再邀请必须 Err");
    assert!(matches!(err, ChatError::AlreadyFriends(_)));
    let self_err = a
        .chat
        .friend_invite(&peer_str(&a.node), "me", vec![], None)
        .await
        .expect_err("邀请自己必须 Err");
    assert!(matches!(self_err, ChatError::SelfPeer(_)));
    cleanup(&a);
    cleanup(&b);
}

#[tokio::test]
async fn accept_without_pending_invite_is_not_found() {
    let a = spawn("inv-none-a").await;
    let b = spawn("inv-none-b").await;
    let err = b
        .chat
        .invite_accept(&peer_str(&a.node), "x")
        .await
        .expect_err("无来邀同意必须 Err");
    assert!(matches!(err, ChatError::NotFound(_)));
    let err = b.chat.invite_reject(&peer_str(&a.node)).await.expect_err("无来邀拒绝必须 Err");
    assert!(matches!(err, ChatError::NotFound(_)));
    cleanup(&a);
    cleanup(&b);
}

#[tokio::test]
async fn undeliverable_invite_stays_pending_locally() {
    let a = spawn("inv-pend-a").await;
    let b = spawn("inv-pend-b").await;
    let b_peer = peer_str(&b.node);
    // 不登记任何地址：连接必失败，邀请挂起（delivered=false），好友簿不变
    let report = a.chat.friend_invite(&b_peer, "小 b", vec![], None).await.expect("invite");
    assert!(!report.delivered, "无地址不得宣称送达");
    assert!(
        invite_of(&a, &b_peer, InviteDirection::Out).await.is_some(),
        "投递失败邀请保持挂起"
    );
    assert!(!has_friend(&a, &b_peer));
    // 对端在线后凭地址重发：同一条目送达
    let report = a
        .chat
        .friend_invite(&b_peer, "小 b", b.node.listen_addrs(), None)
        .await
        .expect("invite");
    assert!(report.delivered, "补登记地址后重发应送达");
    assert!(
        invite_of(&a, &b_peer, InviteDirection::Out).await.is_some(),
        "送达后仍保持挂起，直至对方同意"
    );
    cleanup(&a);
    cleanup(&b);
}
