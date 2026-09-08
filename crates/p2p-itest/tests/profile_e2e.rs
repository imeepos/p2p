//! /im/profile/1 全链回环 itest：双真实 Node，资料查询按对端本机供给应答。
//! 覆盖：具名资料互通 / 缺省装配回空（非错误）/ 离线节点查询报连接失败。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use p2p::Node;
use p2p_chat::{Chat, PeerProfile};

/// 单步等待上限：本地 loopback 全链往返毫秒级，15s 是宽松护栏（同 chat_e2e）。
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

fn provider(tag: &'static str) -> p2p_chat::LocalProfileFn {
    Arc::new(move || PeerProfile {
        name: tag.into(),
        description: format!("{tag} 的自我介绍"),
        avatar: None,
    })
}

/// 起双节点并各自带具名资料装配 Chat。
#[rustfmt::skip]
async fn rig(tag: &str) -> Rig {
    let _ = p2p_log::init(Default::default());
    let root = std::env::temp_dir().join(format!("profile-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let a_dir = root.join("a");
    let b_dir = root.join("b");
    let a_node = build_node(a_dir.clone()).await;
    let a_chat = Chat::with_local_profile(a_node.clone(), a_dir, provider("节点A")).unwrap();
    let b_node = build_node(b_dir.clone()).await;
    let b_chat = Chat::with_local_profile(b_node.clone(), b_dir, provider("节点B")).unwrap();
    Rig { a_node, b_node, a_chat, b_chat, root }
}

#[rustfmt::skip]
fn tcp_addrs(node: &Node) -> Vec<String> {
    node.listen_addrs().into_iter().filter(|a| a.contains("/t")).collect()
}

fn teardown(r: Rig) {
    r.a_node.shutdown();
    r.b_node.shutdown();
    let _ = std::fs::remove_dir_all(&r.root);
}

/// 场景 a：A 查 B 资料按 B 供给应答；B 查 A 同理（双向互通，无需好友关系）。
#[tokio::test]
async fn profile_roundtrip_between_peers() {
    let r = rig("roundtrip").await;
    let a_peer = r.a_node.local_peer_id().to_string();
    let b_peer = r.b_node.local_peer_id().to_string();
    // 借 friend_add 登记地址簿（协议本身不要求好友关系）
    r.a_chat
        .friend_add(&b_peer, "b", tcp_addrs(&r.b_node), None)
        .unwrap();
    r.b_chat
        .friend_add(&a_peer, "a", tcp_addrs(&r.a_node), None)
        .unwrap();
    let from_a = tokio::time::timeout(STEP, r.a_chat.peer_profile(&b_peer))
        .await
        .expect("A 查 B 超时")
        .unwrap();
    assert_eq!(from_a.name, "节点B");
    assert_eq!(from_a.description, "节点B 的自我介绍");
    let from_b = tokio::time::timeout(STEP, r.b_chat.peer_profile(&a_peer))
        .await
        .expect("B 查 A 超时")
        .unwrap();
    assert_eq!(from_b.name, "节点A");
    teardown(r);
}

/// 场景 b：缺省装配（Chat::new）供给空资料——查询成功返回全空，非错误。
#[tokio::test]
async fn default_assembly_answers_empty_profile() {
    let r = rig("empty").await;
    let b_peer = r.b_node.local_peer_id().to_string();
    let a_peer = r.a_node.local_peer_id().to_string();
    let b_dir = r.root.join("b");
    r.a_chat
        .friend_add(&b_peer, "b", tcp_addrs(&r.b_node), None)
        .unwrap();
    r.b_node.shutdown();
    // 同 data_dir 重启 B（身份不变），缺省装配 Chat::new
    let node = build_node(b_dir.clone()).await;
    let _b_chat = Chat::new(node.clone(), b_dir).unwrap();
    let addrs = tcp_addrs(&node);
    r.a_chat.friend_remove(&b_peer).unwrap();
    r.a_chat.friend_add(&b_peer, "b", addrs, None).unwrap();
    let _ = a_peer;
    let profile = tokio::time::timeout(STEP, r.a_chat.peer_profile(&b_peer))
        .await
        .expect("查空资料超时")
        .unwrap();
    assert_eq!(profile, PeerProfile::default());
    node.shutdown();
    teardown(r);
}

/// 场景 c：查询不可达节点 → 连接失败（可读中文 Err，非 panic）。
#[tokio::test]
async fn query_unreachable_peer_fails_readable() {
    let r = rig("unreachable").await;
    let b_peer = r.b_node.local_peer_id().to_string();
    r.a_chat
        .friend_add(&b_peer, "b", tcp_addrs(&r.b_node), None)
        .unwrap();
    r.b_node.shutdown();
    let err = tokio::time::timeout(STEP, r.a_chat.peer_profile(&b_peer))
        .await
        .expect("离线查询超时")
        .expect_err("离线节点必须报错");
    assert!(err.to_string().contains("连接失败"), "错误可读：{err}");
    teardown(r);
}
