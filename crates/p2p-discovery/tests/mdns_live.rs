//! 真实组播集成测试：本机手动运行（CI 不跑，依赖本机多播可用）。
//! 手动命令：cargo test -p p2p-discovery --test mdns_live -- --ignored

use std::sync::Arc;
use std::time::Duration;

use p2p_discovery::{Discovery, DiscoveryEvent, MdnsConfig, MdnsDiscovery};
use p2p_identity::{Keypair, PeerId};
use tokio::sync::mpsc;

async fn wait_discovered(rx: &mut mpsc::Receiver<DiscoveryEvent>, peer: PeerId) -> bool {
    tokio::time::timeout(Duration::from_secs(20), async {
        while let Some(ev) = rx.recv().await {
            if matches!(ev, DiscoveryEvent::Discovered(dp) if dp.peer == peer) {
                return;
            }
        }
    })
    .await
    .is_ok()
}

#[tokio::test]
#[ignore = "requires real multicast on the host; run manually"]
async fn two_nodes_discover_each_other() {
    let peer_a = Keypair::generate().peer_id();
    let peer_b = Keypair::generate().peer_id();
    let disc_a = Arc::new(MdnsDiscovery::new(MdnsConfig::new(peer_a)));
    let disc_b = Arc::new(MdnsDiscovery::new(MdnsConfig::new(peer_b)));

    let (tx_a, mut rx_a) = mpsc::channel(32);
    let (tx_b, mut rx_b) = mpsc::channel(32);
    let (a, b) = (disc_a.clone(), disc_b.clone());
    tokio::spawn(async move { a.run(tx_a).await });
    tokio::spawn(async move { b.run(tx_b).await });

    assert!(wait_discovered(&mut rx_a, peer_b).await, "A 应发现 B");
    assert!(wait_discovered(&mut rx_b, peer_a).await, "B 应发现 A");
}
