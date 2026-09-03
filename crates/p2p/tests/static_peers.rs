//! 静态对端登记端到端（社交化发现 P1 验收）：
//! 1. upsert 即进地址簿（Manual 来源，事件可观测）且落盘；
//! 2. 重启新实例从文件恢复登记——「断网重启后好友可直拨」的地址簿前提；
//! 3. query_peer 在 bootstrap 未接线时显式报错。

use p2p::{NodeBuilder, NodeEvent};
use std::path::PathBuf;

fn temp_file(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{tag}-{}.json", std::process::id()))
}

fn manual_builder(tag: &str, path: PathBuf) -> NodeBuilder {
    // data_dir 显式指到临时目录：默认 ./p2p-data 会在 crate 目录落 key.seed
    NodeBuilder::new()
        .mdns(false)
        .static_peers_file(path)
        .data_dir(std::env::temp_dir().join(format!("{tag}-data-{}", std::process::id())))
}

#[tokio::test]
async fn upsert_registers_and_persists_across_restart() {
    let path = temp_file("p2p-static-peers-it");
    let _ = std::fs::remove_file(&path);

    let node = manual_builder("upsert", path.clone())
        .build()
        .await
        .expect("build ok");
    let peer = p2p::PeerId::from_bytes([7u8; 32]);
    let peer_str = peer.to_string();
    let mut rx = node.events();
    node.upsert_static_peer(&peer_str, vec!["10.0.0.5/u41000".into()], "friend")
        .expect("upsert ok");
    match rx.try_recv().expect("登记必须发 PeerDiscovered") {
        NodeEvent::PeerDiscovered {
            peer: p, source, ..
        } => {
            assert_eq!(p, peer);
            assert!(format!("{source:?}").contains("Manual"));
        }
        other => panic!("expected PeerDiscovered, got {other:?}"),
    }
    node.shutdown();

    // 重启新实例：文件恢复登记，无任何发现源参与
    let node2 = manual_builder("restart", path.clone())
        .build()
        .await
        .expect("rebuild ok");
    assert!(node2.peer_registered(&peer), "重启后必须从文件恢复静态登记");
    let addrs = node2.peer_addrs(&peer);
    assert_eq!(addrs, vec!["10.0.0.5/u41000".to_string()]);
    node2.shutdown();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn query_peer_without_bootstrap_is_explicit_error() {
    let node = NodeBuilder::new()
        .mdns(false)
        .data_dir(std::env::temp_dir().join(format!("p2p-sp-qp-data-{}", std::process::id())))
        .build()
        .await
        .expect("build ok");
    let err = node
        .query_peer(&p2p::PeerId::from_bytes([1u8; 32]).to_string())
        .await
        .expect_err("bootstrap 未接线必须显式报错");
    assert!(err.to_string().contains("rendezvous not wired"));
    node.shutdown();
}
