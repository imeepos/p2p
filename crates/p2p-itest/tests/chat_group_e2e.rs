//! 群聊全链 itest（G5）：roles A(owner)/B/C 真实三 Node + p2p-chat 群门面。
//! 全链：建群→roster→文本→附件→离线成员上线 flush→kick→leave→重邀回归→解散。
//! 端口/身份纪律同 chat_e2e：端口 0 + mDNS 关；data_dir 内 key.seed 重启同身份。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use p2p::{Node, NodeEvent};
use p2p_chat::{ChatKind, ChatMediaInput, GroupEvent, GroupState};
use tokio::sync::broadcast;

const STEP: Duration = Duration::from_secs(15);

#[rustfmt::skip]
struct Role { node: Arc<Node>, dir: PathBuf, chat: p2p_chat::Chat, events: broadcast::Receiver<GroupEvent> }
#[rustfmt::skip]
struct Rig { a: Role, b: Role, c: Role, root: PathBuf }

#[rustfmt::skip]
async fn build_node(dir: PathBuf) -> Arc<Node> {
    Arc::new(Node::builder().mdns(false).data_dir(dir).build().await.unwrap())
}

#[rustfmt::skip]
async fn role(root: &Path, name: &str) -> Role {
    let dir = root.join(name);
    let node = build_node(dir.clone()).await;
    let chat = p2p_chat::Chat::new(node.clone(), dir.clone()).unwrap();
    let events = chat.group_events();
    Role { node, dir, chat, events }
}

#[rustfmt::skip]
async fn rig(tag: &str) -> Rig {
    let _ = p2p_log::init(Default::default());
    let root = std::env::temp_dir().join(format!("group-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let a = role(&root, "a").await;
    let b = role(&root, "b").await;
    let c = role(&root, "c").await;
    Rig { a, b, c, root }
}

#[rustfmt::skip]
fn tcp_addrs(node: &Node) -> Vec<String> {
    node.listen_addrs().into_iter().filter(|a| a.contains("/t")).collect()
}

#[rustfmt::skip]
fn peer(role: &Role) -> String { role.node.local_peer_id().to_string() }

/// owner(A) 与 B/C 加好友并登记 TCP 地址（B/C 无需互识：roster 单向下发）。
#[rustfmt::skip]
async fn friend_star(r: &Rig) {
    let a_peer = peer(&r.a);
    r.a.chat.friend_add(&peer(&r.b), "b", tcp_addrs(&r.b.node), None).unwrap();
    r.a.chat.friend_add(&peer(&r.c), "c", tcp_addrs(&r.c.node), None).unwrap();
    r.b.chat.friend_add(&a_peer, "a", tcp_addrs(&r.a.node), None).unwrap();
    r.c.chat.friend_add(&a_peer, "a", tcp_addrs(&r.a.node), None).unwrap();
}

/// 同 data_dir 重启：身份不变，好友簿/群库/outbox 继承。
#[rustfmt::skip]
async fn restart_role(dir: PathBuf, want_peer: &str) -> Role {
    let node = build_node(dir.clone()).await;
    assert_eq!(node.local_peer_id().to_string(), want_peer, "重启必须保持身份");
    let chat = p2p_chat::Chat::new(node.clone(), dir.clone()).unwrap();
    let events = chat.group_events();
    Role { node, dir, chat, events }
}

/// 事件流等待：命中谓词返回事件；超时 panic（what 留信号）。
#[rustfmt::skip]
async fn wait_event<T: Clone>(rx: &mut broadcast::Receiver<T>, what: &str, mut pred: impl FnMut(&T) -> bool) -> T {
    let deadline = tokio::time::Instant::now() + STEP;
    loop {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Ok(ev)) => { if pred(&ev) { return ev; } }
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
            _ => panic!("事件未在 {STEP:?} 内出现：{what}"),
        }
    }
}

/// 轮询谓词直至成立（群库读路径无事件流时使用）。
#[rustfmt::skip]
async fn wait_until(what: &str, mut pred: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + STEP;
    while !pred() {
        if tokio::time::Instant::now() >= deadline { panic!("条件未在 {STEP:?} 内成立：{what}"); }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn teardown(r: Rig) {
    r.a.node.shutdown();
    r.b.node.shutdown();
    r.c.node.shutdown();
    let _ = std::fs::remove_dir_all(&r.root);
}

#[rustfmt::skip]
fn state_is(role: &Role, group_id: &str, want: GroupState) -> bool {
    role.chat.group.group_list().iter().any(|g| g.group_id == group_id && g.state == want)
}

#[rustfmt::skip]
async fn create_group(r: &Rig, name: &str, members: &[String]) -> p2p_chat::GroupInfo {
    r.a.chat.group.group_create(name, members).await.expect("建群")
}

#[rustfmt::skip]
async fn send_text(r: &Rig, gid: &str, text: &str) -> p2p_chat::GroupSendReport {
    r.a.chat.group.group_send(gid, ChatKind::Text, Some(text.into()), None, None).await.expect("send")
}

#[rustfmt::skip]
async fn send_png(r: &Rig, gid: &str, data: Vec<u8>) -> p2p_chat::GroupSendReport {
    let media = ChatMediaInput { name: "shot.png".into(), mime: "image/png".into(), data };
    r.a.chat.group.group_send(gid, ChatKind::Image, None, Some(media), None).await.expect("send")
}

/// 全链演练（G5 验收序列）：建群→roster→文本→附件→离线成员上线 flush→
/// kick→leave→重邀回归→解散。每步断言两端可观测状态。
#[tokio::test]
#[rustfmt::skip]
async fn group_full_chain_three_nodes() {
    let mut r = rig("chain").await;
    friend_star(&r).await;
    let a_peer = peer(&r.a);
    let b_peer = peer(&r.b);
    let c_peer = peer(&r.c);

    // 1) 建群 → B/C 收 roster（chat_group_state）并落库 active
    let g = create_group(&r, "演练群", &[b_peer.clone(), c_peer.clone()]).await;
    for role in [&mut r.b, &mut r.c] {
        let ev = wait_event(&mut role.events, "成员收 roster", |e| matches!(e, GroupEvent::State { group } if group.group_id == g.group_id)).await;
        let GroupEvent::State { group } = ev else { unreachable!() };
        assert_eq!(group.owner, a_peer, "成员视角 owner = A");
        assert_eq!(group.state, GroupState::Active);
    }

    // 2) 文本 fan-out：B/C 收 chat_group_message；A 侧 acks 到齐 → delivered
    let report = send_text(&r, &g.group_id, "第一条").await;
    assert!(report.delivered, "两在线成员实时送达");
    for role in [&mut r.b, &mut r.c] {
        let ev = wait_event(&mut role.events, "成员收文本", |e| matches!(e, GroupEvent::Message { message, .. } if message.text.as_deref() == Some("第一条"))).await;
        let GroupEvent::Message { message, .. } = ev else { unreachable!() };
        assert_eq!(message.sender_id, a_peer);
        assert!(message.acks.is_empty(), "收到的消息 acks 恒空");
    }

    // 3) 附件 fan-out：B 落盘 media/<groupId>/ 且字节一致
    let png = vec![0x89u8, b'P', b'N', b'G', 9, 8, 7, 6];
    let report = send_png(&r, &g.group_id, png.clone()).await;
    assert!(report.delivered);
    let ev = wait_event(&mut r.b.events, "B 收附件", |e| matches!(e, GroupEvent::Message { message, .. } if message.media.is_some())).await;
    let GroupEvent::Message { message, .. } = ev else { unreachable!() };
    let meta = r.b.chat.group.group_media_file(&g.group_id, &message.id).expect("B 附件路径");
    assert_eq!(std::fs::read(meta.path.expect("B 落盘路径")).expect("读附件"), png, "附件字节一致");

    // 4) 离线成员上线 flush：等 A 感知 C 断开（防复用死连接误判 failed）→ 发文本
    //    （C 条目保持 pending）→ C 重启拨 A → 补投落盘 + acks 到齐
    let mut ev_a_node = r.a.node.events();
    r.c.node.shutdown();
    wait_event(&mut ev_a_node, "A 感知 C 下线", |e| matches!(e, NodeEvent::PeerDisconnected { peer: p } if p.to_string() == c_peer)).await;
    let report = send_text(&r, &g.group_id, "离线补投").await;
    assert!(!report.delivered && report.acked == 1, "仅 B 在线确认");
    r.c = restart_role(r.c.dir.clone(), &c_peer).await;
    r.a.chat.friend_add(&c_peer, "c", tcp_addrs(&r.c.node), None).expect("A 刷新 C 重启后地址");
    r.c.node.connect(r.a.node.local_peer_id()).await.expect("C 拨 A");
    wait_until("C 补投消息落盘", || {
        r.c.chat.group.group_history(&g.group_id, None, 10).is_ok_and(|h| h.iter().any(|m| m.text.as_deref() == Some("离线补投")))
    }).await;
    wait_until("A 侧 acks 到齐", || {
        r.a.chat.group.group_history(&g.group_id, None, 10).is_ok_and(|h| h.iter().all(|m| m.acks.len() == 2))
    }).await;

    // 5) kick B：B 端 state=kicked、禁发
    let g2 = r.a.chat.group.group_kick(&g.group_id, &b_peer).await.expect("kick");
    assert_eq!(g2.rev, 1, "建群 rev0，kick 后 rev=1");
    assert!(!g2.members.contains(&b_peer), "名单已收缩");
    wait_until("B 端 state=kicked", || state_is(&r.b, &g.group_id, GroupState::Kicked)).await;
    let err = r.b.chat.group.group_send(&g.group_id, ChatKind::Text, Some("还想说".into()), None, None).await.expect_err("被踢者禁发");
    assert!(err.to_string().contains("禁止发送"), "err: {err}");

    // 6) leave C：本端 state=left；A 端名单收缩
    let g3 = r.c.chat.group.group_leave(&g.group_id).await.expect("leave");
    assert_eq!(g3.state, GroupState::Left);
    wait_until("A 端名单无 C", || {
        r.a.chat.group.group_list().iter().any(|g| g.group_id == g.group_id && !g.members.contains(&c_peer))
    }).await;

    // 7) 重邀回归：C 收高 rev roster → state 回 active
    r.a.chat.group.group_invite(&g.group_id, &[c_peer.clone()]).await.expect("重邀");
    wait_until("C 端回归 active", || state_is(&r.c, &g.group_id, GroupState::Active)).await;

    // 8) 解散：C 收 G_KICK(disbanded) → disbanded；解散后禁发
    let g5 = r.a.chat.group.group_disband(&g.group_id).await.expect("解散");
    assert_eq!(g5.state, GroupState::Disbanded);
    wait_until("C 端 state=disbanded", || state_is(&r.c, &g.group_id, GroupState::Disbanded)).await;
    let err = r.c.chat.group.group_send(&g.group_id, ChatKind::Text, Some("最后一次".into()), None, None).await.expect_err("解散后禁发");
    assert!(err.to_string().contains("禁止发送"), "err: {err}");
    teardown(r);
}
