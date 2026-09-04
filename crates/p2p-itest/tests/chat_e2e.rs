//! IM 全链回环 itest（T33）：roles A/B 真实双 Node + p2p-chat 门面。
//! 六场景：加好友可见 / 文本实时送达 / ≥1MiB 附件字节一致 / B 重启后 flush /
//!         A 历史回读 / A 引用 B 消息回复（replyTo 透传 + 旧格式行读回兼容）。
//! 端口纪律：NodeBuilder 默认端口 0（随机）+ mDNS 关闭，并行测试不撞口；
//! 身份持久化：data_dir 内 key.seed，重启同 data_dir 即同 PeerId（design §4）。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use p2p::{Node, NodeEvent};
use p2p_chat::{
    Chat, ChatEnvelope, ChatEvent, ChatKind, ChatMediaInput, ChatSendReport, ChatStatus,
};
use tokio::sync::broadcast;

/// 单步等待上限：本地 loopback 全链往返毫秒级，15s 是宽松护栏。
const STEP: Duration = Duration::from_secs(15);

struct Rig {
    a_node: Arc<Node>,
    b_node: Arc<Node>,
    a_chat: Chat,
    b_chat: Chat,
    root: PathBuf,
}

#[rustfmt::skip]
async fn build_node(dir: PathBuf) -> Arc<Node> {
    Arc::new(Node::builder().mdns(false).data_dir(dir).build().await.unwrap())
}

/// 起双节点并各自装配 Chat（好友簿/outbox/handler 一次到位）。
#[rustfmt::skip]
async fn rig(tag: &str) -> Rig {
    let _ = p2p_log::init(Default::default());
    let root = std::env::temp_dir().join(format!("chat-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let a_dir = root.join("a");
    let b_dir = root.join("b");
    let a_node = build_node(a_dir.clone()).await;
    let a_chat = Chat::new(a_node.clone(), a_dir).unwrap();
    let b_node = build_node(b_dir.clone()).await;
    let b_chat = Chat::new(b_node.clone(), b_dir).unwrap();
    Rig { a_node, b_node, a_chat, b_chat, root }
}

#[rustfmt::skip]
fn tcp_addrs(node: &Node) -> Vec<String> {
    node.listen_addrs().into_iter().filter(|a| a.contains("/t")).collect()
}

#[rustfmt::skip]
async fn friend_both(r: &Rig) {
    let a_peer = r.a_node.local_peer_id().to_string();
    let b_peer = r.b_node.local_peer_id().to_string();
    r.a_chat.friend_add(&b_peer, "b", tcp_addrs(&r.b_node), None).unwrap();
    r.b_chat.friend_add(&a_peer, "a", tcp_addrs(&r.a_node), None).unwrap();
}

#[rustfmt::skip]
async fn send_text(chat: &Chat, peer: &str, text: &str, reply_to: Option<String>) -> ChatSendReport {
    chat.send(peer, ChatKind::Text, Some(text.into()), None, reply_to).await.unwrap()
}

#[rustfmt::skip]
async fn send_media(chat: &Chat, peer: &str, data: Vec<u8>) -> ChatSendReport {
    let media = ChatMediaInput { name: "pic.png".into(), mime: "image/png".into(), data };
    chat.send(peer, ChatKind::Image, None, Some(media), None).await.unwrap()
}

/// 同 data_dir 重启节点并重装配 Chat：身份不变，历史/好友簿/outbox 继承。
#[rustfmt::skip]
async fn restart_chat(dir: PathBuf, want_peer: &str) -> (Arc<Node>, Chat) {
    let node = build_node(dir.clone()).await;
    assert_eq!(node.local_peer_id().to_string(), want_peer, "重启必须保持身份");
    let chat = Chat::new(node.clone(), dir).unwrap();
    (node, chat)
}

/// 事件流等待：命中谓词返回 Some(event)；超时 panic（what 留信号）。
async fn wait_event<T: Clone>(
    rx: &mut broadcast::Receiver<T>,
    what: &str,
    mut pred: impl FnMut(&T) -> bool,
) -> Option<T> {
    let deadline = tokio::time::Instant::now() + STEP;
    loop {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Ok(ev)) => {
                if pred(&ev) {
                    return Some(ev);
                }
            }
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
            _ => panic!("事件未在 {STEP:?} 内出现：{what}"),
        }
    }
}

/// 等 A 的 swarm 感知 B 断开：shutdown 后连接传播有延迟，先感知再发离线消息，
/// 避免 send 复用未断旧连接而误落 failed 而非 pending。
#[rustfmt::skip]
async fn wait_node_disconnected(node: &Node, peer: &str) {
    let mut rx = node.events();
    wait_event(&mut rx, "A 感知 B 下线", |e| matches!(e, NodeEvent::PeerDisconnected { peer: p } if p.to_string() == peer)).await;
}

/// 等待一条 chat_message 事件并取出信封；超时/非 message 事件 panic。
async fn wait_chat_message(
    rx: &mut broadcast::Receiver<ChatEvent>,
    what: &str,
    pred: impl FnMut(&ChatEvent) -> bool,
) -> ChatEnvelope {
    let ev = wait_event(rx, what, pred)
        .await
        .expect("必须收到 chat_message");
    match ev {
        ChatEvent::ChatMessage { message, .. } => message,
        _ => unreachable!(),
    }
}

fn teardown(r: Rig) {
    r.a_node.shutdown();
    r.b_node.shutdown();
    let _ = std::fs::remove_dir_all(&r.root);
}

/// 场景 a：加好友（合法 peerId+addr）→ 好友簿可见。
#[tokio::test]
async fn friend_add_visible_in_list() {
    let r = rig("friend").await;
    let b_peer = r.b_node.local_peer_id().to_string();
    let friend = r
        .a_chat
        .friend_add(&b_peer, "bob", tcp_addrs(&r.b_node), Some("itest".into()))
        .unwrap();
    assert_eq!(friend.peer_id, b_peer);
    let list = r.a_chat.friends_list().unwrap();
    let has = list
        .iter()
        .any(|f| f.peer_id == b_peer && f.nickname == "bob");
    assert!(has, "好友簿：{list:?}");
    teardown(r);
}

/// 场景 b：文本消息实时送达（delivered=true，B 收 chat_message 事件）。
#[tokio::test]
async fn text_delivered_realtime() {
    let r = rig("text").await;
    friend_both(&r).await;
    let a_peer = r.a_node.local_peer_id().to_string();
    let b_peer = r.b_node.local_peer_id().to_string();
    let mut bev = r.b_chat.events();
    let report = send_text(&r.a_chat, &b_peer, "hello e2e", None).await;
    assert!(report.delivered, "在线文本必须 delivered：{report:?}");
    let message = wait_chat_message(&mut bev, "B 收到文本", |e| {
        matches!(e, ChatEvent::ChatMessage { peer, message } if peer == &a_peer && message.text.as_deref() == Some("hello e2e"))
    })
    .await;
    assert_eq!(message.sender, p2p_chat::Sender::Them);
    teardown(r);
}

/// 场景 c：≥1MiB 附件送达，B 侧 media_file 落盘路径可读且字节一致。
#[tokio::test]
async fn media_delivered_bytes_identical() {
    let r = rig("media").await;
    friend_both(&r).await;
    let a_peer = r.a_node.local_peer_id().to_string();
    let b_peer = r.b_node.local_peer_id().to_string();
    let data: Vec<u8> = (0..(1 << 20) + 123).map(|i| (i % 251) as u8).collect();
    let mut bev = r.b_chat.events();
    let report = send_media(&r.a_chat, &b_peer, data.clone()).await;
    assert!(report.delivered, "在线附件必须 delivered：{report:?}");
    let message = wait_chat_message(&mut bev, "B 收到附件", |e| {
        matches!(e, ChatEvent::ChatMessage { peer, message } if peer == &a_peer && message.media.is_some())
    })
    .await;
    let meta = r.b_chat.media_file(&a_peer, &message.id).unwrap();
    let path = meta.path.expect("对端附件必须已落盘");
    assert_eq!(meta.size, data.len() as u64);
    assert_eq!(std::fs::read(&path).unwrap(), data, "附件字节必须一致");
    teardown(r);
}

/// 场景 d：B 重启（同 data_dir 同身份）→ A 离线消息 pending → B 上线后 outbox flush → delivered。
#[tokio::test]
async fn offline_pending_then_flush_after_restart() {
    let r = rig("restart").await;
    friend_both(&r).await;
    let a_peer = r.a_node.local_peer_id().to_string();
    let b_peer = r.b_node.local_peer_id().to_string();
    let warm = send_text(&r.a_chat, &b_peer, "warm", None).await;
    assert!(warm.delivered, "链路预热必须在线送达");
    r.b_node.shutdown();
    wait_node_disconnected(&r.a_node, &b_peer).await;
    let report = send_text(&r.a_chat, &b_peer, "offline-msg", None).await;
    assert!(!report.delivered, "下线发送不得实时送达：{report:?}");
    assert_eq!(report.message.status, ChatStatus::Pending, "须保持 pending");
    let offline_id = report.message.id.clone();
    // B2 同 data_dir 重启：身份不变，历史/好友簿继承
    let (b2_node, b2_chat) = restart_chat(r.root.join("b"), &b_peer).await;
    // A 补登记 B2 新地址并拨号 → PeerConnected 触发 outbox flush
    for addr in tcp_addrs(&b2_node) {
        r.a_node
            .add_peer_address(b2_node.local_peer_id(), &addr)
            .unwrap();
    }
    let mut aev = r.a_chat.events();
    r.a_node.connect(b2_node.local_peer_id()).await.unwrap();
    wait_event(&mut aev, "A outbox flush 后 Delivered", |e| {
        matches!(e, ChatEvent::ChatStatus { message_id, status: ChatStatus::Delivered, .. } if message_id == &offline_id)
    })
    .await
    .expect("A 必须收到 Delivered 状态");
    let got = b2_chat.history(&a_peer, None, 100).unwrap();
    let has = got
        .iter()
        .any(|m| m.id == offline_id && m.text.as_deref() == Some("offline-msg"));
    assert!(has, "B2 必须收到离线消息：{got:?}");
    b2_node.shutdown();
    teardown(r);
}

/// 场景 e：A 历史回读包含重启前后全部消息（beforeId 分页游标翻到底）。
#[tokio::test]
async fn history_readback_with_pagination() {
    let r = rig("history").await;
    friend_both(&r).await;
    let a_peer = r.a_node.local_peer_id().to_string();
    let b_peer = r.b_node.local_peer_id().to_string();
    let mut want: Vec<String> = Vec::new();
    for i in 0..2 {
        let rep = send_text(&r.a_chat, &b_peer, &format!("a{i}"), None).await;
        assert!(rep.delivered);
        want.push(rep.message.id.clone());
        tokio::time::sleep(Duration::from_millis(10)).await; // 保证 tsMs 严格递增（分页游标确定性）
        let rep = send_text(&r.b_chat, &a_peer, &format!("b{i}"), None).await;
        assert!(rep.delivered);
        want.push(rep.message.id.clone());
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // A 重启（同 data_dir）：历史与身份一起回来
    let (a2_node, a2_chat) = restart_chat(r.root.join("a"), &a_peer).await;
    let all = a2_chat.history(&b_peer, None, 100).unwrap();
    assert_eq!(all.len(), want.len(), "重启后必须包含全部消息：{all:?}");
    // beforeId 分页：limit=2 逐页翻到底，id 集合与全量一致
    let mut got: Vec<String> = Vec::new();
    let mut before: Option<String> = None;
    loop {
        let page = a2_chat.history(&b_peer, before.as_deref(), 2).unwrap();
        if page.is_empty() {
            break;
        }
        got.extend(page.iter().map(|m| m.id.clone()));
        before = Some(page.last().unwrap().id.clone());
    }
    let mut w = want.clone();
    let mut g = got.clone();
    w.sort();
    g.sort();
    assert_eq!(g, w, "分页必须覆盖全部消息：{got:?} vs {want:?}");
    a2_node.shutdown();
    teardown(r);
}

/// 场景 f：A 引用 B 的消息回复，B 端信封 replyTo 指向被引用 id；旧格式行（无 replyTo）读回兼容。
#[tokio::test]
#[rustfmt::skip]
async fn reply_reference_and_legacy_envelope_readback() {
    let r = rig("reply").await;
    friend_both(&r).await;
    let a_peer = r.a_node.local_peer_id().to_string();
    let b_peer = r.b_node.local_peer_id().to_string();
    let quoted = send_text(&r.b_chat, &a_peer, "quote-me", None).await;
    assert!(quoted.delivered, "被引用消息必须先送达");
    let mut bev = r.b_chat.events();
    let reply = send_text(&r.a_chat, &b_peer, "reply", Some(quoted.message.id.clone())).await;
    assert_eq!(reply.message.reply_to.as_deref(), Some(quoted.message.id.as_str()));
    let inbound = wait_chat_message(&mut bev, "B 收到回复信封", |e| {
        matches!(e, ChatEvent::ChatMessage { message, .. } if message.id == reply.message.id)
    }).await;
    assert_eq!(inbound.reply_to.as_deref(), Some(quoted.message.id.as_str()));
    // 旧格式（无 replyTo 字段）行混入 A 的历史文件：重启读回不报错且无引用
    let legacy = serde_json::json!({"id": "legacy-1", "peer": b_peer, "sender": "me",
        "kind": "text", "tsMs": 1, "text": "old", "media": null, "status": "pending"}).to_string();
    let path = r.root.join("a/chat/messages").join(format!("{b_peer}.jsonl"));
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    use std::io::Write as _;
    f.write_all(legacy.as_bytes()).unwrap();
    let (a2_node, a2_chat) = restart_chat(r.root.join("a"), &a_peer).await;
    let legacy_msg = a2_chat.history(&b_peer, None, 100).unwrap().into_iter()
        .find(|m| m.id == "legacy-1").expect("旧格式行必须读回");
    assert_eq!(legacy_msg.reply_to, None, "缺 replyTo 读回 = 无引用");
    a2_node.shutdown();
    teardown(r);
}
