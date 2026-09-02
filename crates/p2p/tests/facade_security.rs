//! M4 安全回归（docs/notes/security-review-1.md）：地址簿投毒不能落地中间人。
//! 按 PeerId 拨号恒带 expected（握手后身份比对），投毒地址只会得到身份不匹配错误。

use std::path::PathBuf;
use std::time::Duration;

use p2p::{Node, ProtocolId};

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-facade-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    dir
}

fn tcp_addrs(node: &Node) -> Vec<String> {
    node.listen_addrs()
        .into_iter()
        .filter(|a| a.contains("/t"))
        .collect()
}

/// 投毒场景：攻击者 C 的地址被登记到受害者 A 的 PeerId 下。
/// connect(A) 必须以身份不匹配失败（C 出示合法但不同的身份），后续 request 同样失败；
/// 投毒地址不能冒充 A（expected 强制绑定，security-review-1.md M4）。
#[tokio::test]
async fn poisoned_addr_book_rejected_by_expected_binding() {
    let a_dir = tmp_dir("m4-a");
    let c_dir = tmp_dir("m4-c");
    let b_dir = tmp_dir("m4-b");
    let node_a = Node::builder()
        .mdns(false)
        .data_dir(a_dir.clone())
        .build()
        .await
        .expect("node a");
    let node_c = Node::builder()
        .mdns(false)
        .data_dir(c_dir.clone())
        .build()
        .await
        .expect("node c");
    let node_b = Node::builder()
        .mdns(false)
        .data_dir(b_dir.clone())
        .build()
        .await
        .expect("node b");
    let a_peer = node_a.local_peer_id();
    let c_peer = node_c.local_peer_id();
    assert_ne!(a_peer, c_peer);

    // 投毒：把 C 的地址登记到 A 的 PeerId 下
    for addr in tcp_addrs(&node_c) {
        node_b
            .add_peer_address(a_peer, &addr)
            .expect("seed poisoned addr");
    }

    node_b
        .connect(a_peer)
        .await
        .expect_err("poisoned dial must fail");
    let outcome = node_b
        .request(
            a_peer,
            ProtocolId::new("/test/echo/1").expect("id"),
            b"x".to_vec(),
            Duration::from_secs(5),
        )
        .await;
    assert!(outcome.is_err(), "poisoned request must fail");

    node_a.shutdown();
    node_c.shutdown();
    node_b.shutdown();
    let _ = std::fs::remove_dir_all(&a_dir);
    let _ = std::fs::remove_dir_all(&c_dir);
    let _ = std::fs::remove_dir_all(&b_dir);
}
