//! 重复发现回归：限频重发 PeerDiscovered + 活跃度账本随重复发现恢复。

use super::tests::test_config;
use super::*;
use crate::lifecycle::LifecycleEvent;
use std::time::Duration;

fn routable_addrs() -> Vec<TransportAddr> {
    vec![TransportAddr::Tcp {
        ip: "192.0.2.10".parse().expect("valid ip"),
        port: 41000,
    }]
}

#[tokio::test]
async fn repeat_discovery_reemits_only_outside_window() {
    let swarm = Swarm::start(test_config()).await.expect("bind swarm");
    swarm.refresh_gate.set_interval(Duration::from_secs(3600));
    let peer = PeerId::from_bytes([9u8; 32]);
    let mut rx = swarm.subscribe();

    swarm.add_peer_addresses_with_source(peer, routable_addrs(), AddrSource::Mdns);
    let first = rx.try_recv().expect("首见必须发 PeerDiscovered");
    assert!(matches!(first, NodeEvent::PeerDiscovered { .. }));

    swarm.add_peer_addresses_with_source(peer, routable_addrs(), AddrSource::Mdns);
    assert!(rx.try_recv().is_err(), "窗口内重复发现不得重发");

    swarm.refresh_gate.set_interval(Duration::ZERO);
    swarm.add_peer_addresses_with_source(peer, routable_addrs(), AddrSource::Mdns);
    match rx.try_recv().expect("窗口外重复发现必须重发") {
        NodeEvent::PeerDiscovered {
            peer: p,
            addrs,
            source,
        } => {
            assert_eq!(p, peer);
            assert_eq!(addrs.len(), 1, "重发携带地址簿现有地址");
            assert!(addrs[0].contains("41000"));
            assert_eq!(source, AddrSource::Mdns);
        }
        other => panic!("expected PeerDiscovered, got {other:?}"),
    }
}

#[tokio::test]
async fn repeat_discovery_recovers_liveness_after_expiry() {
    let swarm = Swarm::start(test_config()).await.expect("bind swarm");
    swarm.refresh_gate.set_interval(Duration::ZERO);
    let peer = PeerId::from_bytes([7u8; 32]);
    let mut lc = swarm.subscribe_lifecycle();

    swarm.add_peer_addresses_with_source(peer, routable_addrs(), AddrSource::Mdns);
    // 首见 Unknown→Alive 只记账不发事件；过期判死（池内无连接）
    swarm.on_peer_expired(peer);
    match lc.try_recv().expect("判死必须发 PeerLiveness") {
        LifecycleEvent::PeerLiveness(p) => assert!(!p.alive),
        other => panic!("expected PeerLiveness dead, got {other:?}"),
    }

    // 修复点：地址无新增的重复发现也刷新活跃度 → Dead→Alive 恢复事件
    swarm.add_peer_addresses_with_source(peer, routable_addrs(), AddrSource::Mdns);
    let ev = tokio::time::timeout(Duration::from_millis(500), lc.recv())
        .await
        .expect("重复发现必须恢复活跃度")
        .expect("channel open");
    match ev {
        LifecycleEvent::PeerLiveness(p) => {
            assert!(p.alive, "重复发现必须把 Dead 翻回 Alive");
            assert_eq!(p.peer, peer);
        }
        other => panic!("expected PeerLiveness alive, got {other:?}"),
    }
}
