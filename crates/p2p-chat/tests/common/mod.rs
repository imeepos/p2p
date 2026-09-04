//! 双节点回环 itest 共享工具：真实 Node 实例 + Chat 门面装配。
//! 断言允许 expect/panic（tests 目录豁免 panic-hygiene）；行数仍受 300 红线约束。
//! 各测试二进制分别编译本模块，未全部用到的辅助函数按 dead_code 放行。
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use p2p::{Node, NodeEvent};
use p2p_chat::{Chat, ChatEvent};
use tokio::sync::broadcast;

pub const WAIT: Duration = Duration::from_secs(30);

/// 测试节点：Node + Chat + 数据目录（身份与聊天存储同目录）。
pub struct TestNode {
    pub node: Arc<Node>,
    pub chat: Chat,
    pub dir: PathBuf,
}

/// 全新目录建节点（端口 0 由内核动态分配：固定端口被其他进程占用时
/// 监听绑定 AddrInUse，用例假红，属潜伏 flake）。
pub async fn spawn(tag: &str) -> TestNode {
    let dir = std::env::temp_dir().join(format!("p2p-chat-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    spawn_at(tag, &dir).await
}

/// 复用既有目录建节点（同 data_dir = 同身份；重启场景用）。
/// 端口 0 动态分配：重启后端口必变，测试需把节点新 listen_addrs 用
/// friend_add（upsert）刷新进对端地址簿后才可拨。
pub async fn spawn_at(_tag: &str, dir: &Path) -> TestNode {
    let node = Arc::new(
        Node::builder()
            .mdns(false)
            .quic_port(0)
            .tcp_port(0)
            .data_dir(dir.join("node"))
            .build()
            .await
            .expect("build node"),
    );
    // Chat::new 内部在 data_dir 下建 chat/ 子目录：传入基础目录，存储落在 dir/chat/
    let chat = Chat::new(node.clone(), dir.to_path_buf()).expect("chat init");
    TestNode {
        node,
        chat,
        dir: dir.to_path_buf(),
    }
}

pub fn peer_str(node: &Node) -> String {
    node.local_peer_id().to_string()
}

pub fn parse_peer(s: &str) -> p2p_identity::PeerId {
    let bytes = bs58::decode(s).into_vec().expect("base58 decode");
    p2p_identity::PeerId::from_bytes(bytes.try_into().expect("32 bytes"))
}

/// 互相加好友并登记地址（可拨）。
pub async fn add_each_other(a: &TestNode, b: &TestNode) {
    a.chat
        .friend_add_direct(&peer_str(&b.node), "b", b.node.listen_addrs(), None)
        .expect("a add b");
    b.chat
        .friend_add_direct(&peer_str(&a.node), "a", a.node.listen_addrs(), None)
        .expect("b add a");
}

/// 等事件通道出现满足谓词的事件（带超时，超时即 panic）。
pub async fn wait_event<T: Clone>(
    rx: &mut broadcast::Receiver<T>,
    want: impl Fn(&T) -> bool,
    what: &str,
) -> T {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let budget = deadline.saturating_duration_since(tokio::time::Instant::now());
        let ev = tokio::time::timeout(budget, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
            .expect("event channel open");
        if want(&ev) {
            return ev;
        }
    }
}

/// 轮询等待条件成立（每 100ms 一次，超时 panic）。
pub async fn wait_until(what: &str, mut pred: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        if pred() {
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for {what}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// 关停节点并清理目录。
pub fn cleanup(node: &TestNode) {
    node.node.shutdown();
    let _ = std::fs::remove_dir_all(&node.dir);
}

/// 订阅 NodeEvent 的便捷入口（测试断言连接事件用）。
pub fn node_events(node: &Node) -> broadcast::Receiver<NodeEvent> {
    node.events()
}

/// 读取 outbox 文件行数（不存在或空 = 无待发条目）。
pub fn outbox_lines(node: &TestNode, peer: &str) -> usize {
    let path = node.dir.join("chat/outbox").join(format!("{peer}.jsonl"));
    match std::fs::read_to_string(&path) {
        Ok(c) => c.lines().count(),
        Err(_) => 0,
    }
}

/// 断言 ChatEvent 已送达（chat_message 且 peer 匹配）。
pub fn is_message_from(ev: &ChatEvent, peer: &str) -> bool {
    matches!(ev, ChatEvent::ChatMessage { peer: p, .. } if p == peer)
}
