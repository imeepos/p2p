//! 同意制入群邀请全链 itest（IMC1）：roles A(owner)/B 真实双 Node + p2p-chat 门面。
//! 全链：owner 向离线好友发起（卡片待投递 + out 落盘）→ 好友上线收卡与 in 向邀请
//! 事件 → 拒绝（owner 侧 rejected，roster 不变）→ owner 重发 → 好友同意 → 双方
//! roster 均含好友、owner 侧 accepted、卡片历史可回放、事件序列断言。
//! 端口/身份纪律同 chat_group_e2e：端口 0 + mDNS 关；data_dir 内 key.seed 重启同身份。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use p2p::Node;
use p2p_chat::{ChatEvent, ChatKind, GroupInviteDirection, GroupInviteState};
use tokio::sync::broadcast;

const STEP: Duration = Duration::from_secs(15);

#[rustfmt::skip]
struct Role { node: Arc<Node>, dir: PathBuf, chat: p2p_chat::Chat, sink: Arc<Mutex<Vec<ChatEvent>>> }
#[rustfmt::skip]
struct Rig { a: Role, b: Role, root: PathBuf }

#[rustfmt::skip]
async fn build_node(dir: PathBuf) -> Arc<Node> {
    Arc::new(Node::builder().mdns(false).data_dir(dir).build().await.unwrap())
}

/// 事件收集器：sink 记录全量事件（订阅先行，断言在终态 wait 之后）。
#[rustfmt::skip]
fn collect(mut rx: broadcast::Receiver<ChatEvent>) -> Arc<Mutex<Vec<ChatEvent>>> {
    let sink = Arc::new(Mutex::new(Vec::new()));
    let clone = sink.clone();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(ev) => clone.lock().unwrap_or_else(|e| e.into_inner()).push(ev),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    sink
}

#[rustfmt::skip]
async fn role(root: &Path, name: &str) -> Role {
    let dir = root.join(name);
    let node = build_node(dir.clone()).await;
    let chat = p2p_chat::Chat::new(node.clone(), dir.clone()).unwrap();
    let sink = collect(chat.events());
    Role { node, dir, chat, sink }
}

#[rustfmt::skip]
fn tcp_addrs(node: &Node) -> Vec<String> {
    node.listen_addrs().into_iter().filter(|a| a.contains("/t")).collect()
}

#[rustfmt::skip]
fn peer(role: &Role) -> String { role.node.local_peer_id().to_string() }

/// 同 data_dir 重启：身份不变，好友簿/邀请簿继承。
#[rustfmt::skip]
async fn restart_role(dir: PathBuf, want_peer: &str) -> Role {
    let node = build_node(dir.clone()).await;
    assert_eq!(node.local_peer_id().to_string(), want_peer, "重启必须保持身份");
    let chat = p2p_chat::Chat::new(node.clone(), dir.clone()).unwrap();
    let sink = collect(chat.events());
    Role { node, dir, chat, sink }
}

/// 轮询谓词直至成立（收敛无事件流侧使用）。
#[rustfmt::skip]
async fn wait_until(what: &str, mut pred: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + STEP;
    while !pred() {
        if tokio::time::Instant::now() >= deadline { panic!("条件未在 {STEP:?} 内成立：{what}"); }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[rustfmt::skip]
fn invite_events(role: &Role) -> Vec<(GroupInviteDirection, GroupInviteState)> {
    role.sink.lock().unwrap_or_else(|e| e.into_inner()).iter()
        .filter_map(|ev| match ev {
            ChatEvent::GroupInvite { invite } => Some((invite.direction, invite.state)),
            _ => None,
        })
        .collect()
}

#[rustfmt::skip]
fn entry_state(role: &Role, id: &str) -> Option<GroupInviteState> {
    role.chat.group.group_invites_list().expect("邀请列表").into_iter()
        .find(|i| i.id == id)
        .map(|i| i.state)
}

/// B 侧 in 向条目（与 owner 侧 out 条目 id 相互独立，按方向定位）。
#[rustfmt::skip]
fn b_in_invite(role: &Role) -> Option<p2p_chat::GroupInvite> {
    role.chat.group.group_invites_list().expect("邀请列表").into_iter()
        .find(|i| i.direction == GroupInviteDirection::In)
}

fn teardown(r: Rig) {
    r.a.node.shutdown();
    r.b.node.shutdown();
    let _ = std::fs::remove_dir_all(&r.root);
}

/// 全链演练（IMC1 验收序列）。
#[tokio::test]
#[rustfmt::skip]
async fn group_invite_consent_full_chain_two_nodes() {
    let _ = p2p_log::init(Default::default());
    let root = std::env::temp_dir().join(format!("gic-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let a = role(&root, "a").await;
    let b0 = role(&root, "b").await;
    let a_peer = peer(&a);
    let b_peer = peer(&b0);
    a.chat.friend_add(&b_peer, "b", tcp_addrs(&b0.node), None).expect("A 加 B");
    b0.chat.friend_add(&a_peer, "a", tcp_addrs(&a.node), None).expect("B 加 A");

    // 1) owner 向离线好友发起：B 下线，卡片消息待投递 + 邀请 out 落盘
    b0.node.shutdown();
    let g = a.chat.group.group_create("同意制群", &[]).await.expect("建群");
    let gid = g.group_id.clone();
    let report = a.chat.group.group_invite_member(&g.group_id, &b_peer, "群主甲", Some("来呀".into())).await.expect("发起邀请");
    assert!(!report.delivered, "对端离线，邀请帧本轮未送达");
    assert_eq!(report.invite.state, GroupInviteState::Pending);
    assert_eq!(report.invite.direction, GroupInviteDirection::Out);
    let invite_id = report.invite.id.clone();
    let card_of = |env: &p2p_chat::ChatEnvelope| env.card.clone().filter(|_| env.kind == ChatKind::GroupInvite);
    let a_card = a.chat.history(&b_peer, None, 50).expect("A 历史").into_iter().find_map(|m| card_of(&m)).expect("A 侧卡片消息已落盘");
    assert_eq!(a_card.group_id, gid, "卡片载荷逐字：groupId");
    assert_eq!(a_card.group_name, "同意制群", "卡片载荷逐字：groupName");
    assert_eq!(a_card.inviter_nickname, "群主甲", "卡片载荷逐字：inviterNickname");
    assert_eq!(a_card.note.as_deref(), Some("来呀"), "卡片载荷逐字：note");

    // 2) 好友上线：收卡片消息（1:1 离线投递）与 in 向邀请事件（GINVITE 重投）
    let b = restart_role(b0.dir.clone(), &b_peer).await;
    b.node.connect(a.node.local_peer_id()).await.expect("B 拨 A");
    wait_until("B 收卡并登记 in 向邀请", || {
        b.chat.history(&a_peer, None, 50).is_ok_and(|h| h.iter().any(|m| card_of(m).is_some()))
            && b_in_invite(&b).is_some_and(|i| i.state == GroupInviteState::Pending)
    }).await;
    let b_invite_id = b_in_invite(&b).expect("B 侧 in 向条目").id;
    let b_card = b.chat.history(&a_peer, None, 50).expect("B 历史").into_iter().find_map(|m| card_of(&m)).expect("B 侧卡片可回放");
    assert_eq!(b_card.inviter_nickname, "群主甲", "卡片载荷跨网保真");
    // 事件序列：卡片消息先于首条邀请事件（flush 顺序：1:1 outbox → ginvite）
    {
        let sink = b.sink.lock().unwrap_or_else(|e| e.into_inner());
        let card_pos = sink.iter().position(|ev| matches!(ev, ChatEvent::ChatMessage { message, .. } if card_of(message).is_some()));
        let invite_pos = sink.iter().position(|ev| matches!(ev, ChatEvent::GroupInvite { .. }));
        assert!(card_pos.is_some_and(|c| invite_pos.is_some_and(|i| c < i)), "卡片消息必须先于邀请事件");
    }
    assert_eq!(invite_events(&b)[0], (GroupInviteDirection::In, GroupInviteState::Pending), "B 首事件 = in/pending");

    // 3) 拒绝：owner 侧状态 rejected，roster 不变
    let rejected = b.chat.group.group_invite_reject(&b_invite_id, Some("再想想".into())).await.expect("拒绝");
    assert_eq!(rejected.state, GroupInviteState::Rejected);
    wait_until("owner 侧置 rejected", || entry_state(&a, &invite_id) == Some(GroupInviteState::Rejected)).await;
    assert_eq!(invite_events(&a)[0], (GroupInviteDirection::Out, GroupInviteState::Pending), "A 首事件 = out/pending");
    assert_eq!(invite_events(&a)[1], (GroupInviteDirection::Out, GroupInviteState::Rejected), "A 拒绝事件");
    let group_after_reject = a.chat.group.group_list().into_iter().find(|g| g.group_id == gid).expect("群在册");
    assert_eq!(group_after_reject.members.len(), 1, "拒绝后 roster 不变");
    assert_eq!(group_after_reject.rev, 0, "拒绝不触发 rev");

    // 4) owner 重发：条目幂等刷新（id 稳定，state 回 pending），B 在线即时送达
    let resent = a.chat.group.group_invite_member(&g.group_id, &b_peer, "群主甲", None).await.expect("重发");
    assert!(resent.delivered, "B 在线，本轮送达");
    assert_eq!(resent.invite.id, invite_id, "重复邀请条目 id 稳定");
    wait_until("B 侧刷新回 pending", || entry_state(&b, &b_invite_id) == Some(GroupInviteState::Pending)).await;

    // 5) 好友同意：双方 roster 均含好友，owner 侧 accepted，B 侧随 roster 收敛 accepted
    b.chat.group.group_invite_accept(&b_invite_id).await.expect("同意");
    wait_until("owner 侧 accepted 且 roster 含 B", || {
        entry_state(&a, &invite_id) == Some(GroupInviteState::Accepted)
            && a.chat.group.group_list().iter().any(|g| g.group_id == gid && g.members.contains(&b_peer) && g.rev == 1)
    }).await;
    wait_until("B 侧 accepted 且入群 active", || {
        entry_state(&b, &b_invite_id) == Some(GroupInviteState::Accepted)
            && b.chat.group.group_list().iter().any(|g| g.group_id == gid && g.state == p2p_chat::GroupState::Active && g.members.contains(&a_peer) && g.members.contains(&b_peer))
    }).await;

    // 6) 卡片历史可回放 + 事件序列断言
    assert!(a.chat.history(&b_peer, None, 50).expect("A 历史").iter().any(|m| card_of(m).is_some()), "A 侧卡片历史");
    assert!(b.chat.history(&a_peer, None, 50).expect("B 历史").iter().any(|m| card_of(m).is_some()), "B 侧卡片历史");
    let states = |role: &Role| invite_events(role).into_iter().map(|(_, s)| s).collect::<Vec<_>>();
    assert_eq!(states(&a), vec![GroupInviteState::Pending, GroupInviteState::Rejected, GroupInviteState::Pending, GroupInviteState::Accepted], "A 事件序列");
    assert_eq!(states(&b), vec![GroupInviteState::Pending, GroupInviteState::Rejected, GroupInviteState::Pending, GroupInviteState::Accepted], "B 事件序列");
    teardown(Rig { a, b, root });
}

/// inviter 离线时受邀者同意挂起：决策落盘 delivered=false → owner 重启回来 →
/// 受邀者重启自愈重投 GACCEPT → roster 收敛 accepted（IMC1 离线语义）。
#[tokio::test]
#[rustfmt::skip]
async fn group_invite_accept_suspended_while_owner_offline() {
    let _ = p2p_log::init(Default::default());
    let root = std::env::temp_dir().join(format!("gic-off-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let a0 = role(&root, "a").await;
    let mut b = role(&root, "b").await;
    let a_peer = peer(&a0);
    let b_peer = peer(&b);
    a0.chat.friend_add(&b_peer, "b", tcp_addrs(&b.node), None).expect("A 加 B");
    b.chat.friend_add(&a_peer, "a", tcp_addrs(&a0.node), None).expect("B 加 A");

    // owner 发起（B 在线即时送达）→ B 拿到 in 向 pending
    let g = a0.chat.group.group_create("离线同意群", &[]).await.expect("建群");
    let gid = g.group_id.clone();
    let report = a0.chat.group.group_invite_member(&gid, &b_peer, "群主乙", None).await.expect("发起");
    assert!(report.delivered, "B 在线即时送达");
    wait_until("B 拿到 in 向邀请", || {
        b_in_invite(&b).is_some_and(|i| i.state == GroupInviteState::Pending)
    }).await;

    // owner 下线，B 同意：决策落盘（delivered=false 挂起），未入群
    a0.node.shutdown();
    let invite_id = b_in_invite(&b).expect("in 条目").id;
    let accepted = b.chat.group.group_invite_accept(&invite_id).await.expect("同意");
    assert_eq!(accepted.state, GroupInviteState::Pending, "roster 未到保持 pending");
    assert!(!accepted.delivered, "owner 离线，同意帧挂起");
    assert!(b.chat.group.group_list().iter().all(|g| g.group_id != gid), "收敛前未入群");

    // owner 重启回来；刷新 B 好友簿里 A 的地址（重启换端口）→ B 重启自愈重投同意帧
    let a = restart_role(a0.dir.clone(), &a_peer).await;
    b.chat.friend_add(&a_peer, "a", tcp_addrs(&a.node), None).expect("刷新 A 地址");
    b = restart_role(b.dir.clone(), &b_peer).await;
    wait_until("重投后 owner 侧 accepted", || {
        entry_state(&a, &report.invite.id) == Some(GroupInviteState::Accepted)
    }).await;
    wait_until("B 侧随 roster 收敛 accepted", || {
        entry_state(&b, &invite_id) == Some(GroupInviteState::Accepted)
            && b.chat.group.group_list().iter().any(|g| g.group_id == gid && g.members.contains(&a_peer) && g.members.contains(&b_peer))
    }).await;
    wait_until("owner roster 含 B", || {
        a.chat.group.group_list().iter().any(|g| g.group_id == gid && g.members.contains(&b_peer) && g.rev == 1)
    }).await;
    teardown(Rig { a, b, root });
}
