//! E5: 指标快照可断言——拨号各跳成败计数、活跃连接水位、成功率的回答能力。

use std::net::IpAddr;
use std::sync::Arc;

use p2p_identity::Keypair;
use p2p_protocol::HandlerRegistry;
use p2p_swarm::{Swarm, SwarmConfig};
use p2p_transport::TransportAddr;

fn config() -> SwarmConfig {
    SwarmConfig {
        keypair: Arc::new(Keypair::generate()),
        quic_port: 0,
        tcp_port: 0,
        registry: Arc::new(HandlerRegistry::default()),
        relay_addrs: Vec::new(),
        advertised_addrs: Vec::new(),
    }
}

#[tokio::test]
async fn metrics_snapshot_counts_direct_hops_and_active_conns() {
    let dialer = Swarm::start(config()).await.expect("dialer");
    let target = Swarm::start(config()).await.expect("target");
    let peer = target.local_peer_id();
    let reachable = target
        .listen_addrs()
        .into_iter()
        .find(|a| matches!(a, TransportAddr::Tcp { .. }))
        .expect("target tcp addr");

    // 首地址必拒（计地址级失败），第二地址可达（计直连跳成功）
    dialer.add_peer_addresses(
        peer,
        vec![
            TransportAddr::Tcp {
                ip: IpAddr::from([127, 0, 0, 1]),
                port: 1,
            },
            reachable,
        ],
    );
    dialer.connect(peer).await.expect("connect must land");

    let snap = dialer.metrics();
    assert!(
        snap.addr_dial_failures >= 1,
        "refused addr must be counted, snap={snap:?}"
    );
    assert_eq!(
        snap.dial_direct_ok, 1,
        "exactly one direct hop success expected, snap={snap:?}"
    );
    assert_eq!(
        snap.dial_direct_fail, 0,
        "direct hop succeeded via second addr, no hop-level failure"
    );
    let rate = snap.dial_direct_success_rate().expect("samples exist");
    assert!((rate - 1.0).abs() < f64::EPSILON);
    assert_eq!(snap.active_connections, 1, "pool holds one connection");
    assert_eq!(snap.relay_sessions_active, 0, "no relay configured");
}

#[tokio::test]
async fn metrics_count_direct_hop_failure_when_all_addrs_fail() {
    let dialer = Swarm::start(config()).await.expect("dialer");
    let target = Swarm::start(config()).await.expect("target");
    let peer = target.local_peer_id();
    dialer.add_peer_addresses(
        peer,
        vec![TransportAddr::Tcp {
            ip: IpAddr::from([127, 0, 0, 1]),
            port: 1,
        }],
    );
    assert!(
        dialer.connect(peer).await.is_err(),
        "single refused addr must fail"
    );
    let snap = dialer.metrics();
    assert_eq!(snap.dial_direct_fail, 1, "exhausted direct hop counts once");
    assert_eq!(snap.addr_dial_failures, 1);
    assert_eq!(snap.active_connections, 0, "failed dial leaves pool empty");
    let rate = snap.dial_direct_success_rate().expect("samples exist");
    assert!(rate.abs() < f64::EPSILON, "0 success rate, snap={snap:?}");
}
