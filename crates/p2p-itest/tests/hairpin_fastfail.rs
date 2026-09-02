//! E4: 同 NAT hairpin 场景——与自身观测地址同公网前缀的对端地址降权并短超时，
//! refused/黑洞不得吃满单地址预算，LAN 地址仍可达。

use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use p2p_identity::Keypair;
use p2p_protocol::HandlerRegistry;
use p2p_swarm::{AddrSource, NodeEvent, Swarm, SwarmConfig};
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

/// 模拟同 NAT 公网侧地址：TEST-NET-3 段，不可路由，拨号必然失败（黑洞或拒绝）。
fn public_quic(port: u16) -> TransportAddr {
    TransportAddr::Quic {
        ip: IpAddr::from([203, 0, 113, 7]),
        port,
    }
}

/// 残余缺陷回归：hairpin 候选（同公网前缀观测地址）登记在 LAN 地址之前时，
/// 直连必须先落地 LAN 地址（本机以环回 TCP 替代 LAN），
/// 整体耗时不得被 hairpin 拨号拖满（旧排序下 QUIC 握手预算 10s 起步）。
#[tokio::test]
async fn hairpin_addr_demoted_lan_connects_first() {
    let dialer = Swarm::start(config()).await.expect("dialer");
    let target = Swarm::start(config()).await.expect("target");
    let peer = target.local_peer_id();
    let lan = target
        .listen_addrs()
        .into_iter()
        .find(|a| matches!(a, TransportAddr::Tcp { .. }))
        .expect("target tcp addr");
    // 拨号方观测到自身公网映射地址 → 对端同前缀地址识别为 hairpin 候选
    dialer.set_observed_addrs(vec![public_quic(45001)]);
    dialer.add_peer_addresses_with_source(peer, vec![public_quic(40000)], AddrSource::Rendezvous);
    dialer.add_peer_addresses_with_source(peer, vec![lan], AddrSource::Rendezvous);

    let mut events = dialer.subscribe();
    let started = Instant::now();
    dialer
        .connect(peer)
        .await
        .expect("lan addr must stay reachable");
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(5),
        "hairpin addr must be demoted so lan dials first, took {elapsed:?}"
    );
    match p2p_itest::expect_within("connect events", events.recv(), Duration::from_secs(2)).await {
        Ok(NodeEvent::PeerConnected { peer: got }) => assert_eq!(got, peer),
        other => panic!("expected PeerConnected after lan dial, got {other:?}"),
    }
}

/// 预算回归：仅剩 hairpin 候选时，拨号须按短超时快速失败并归因可见，
/// 不得占满传输层默认预算（QUIC 握手 10s / macOS UDP 拒绝可达 30s）。
#[tokio::test]
async fn hairpin_only_addr_fails_fast_within_short_budget() {
    let dialer = Swarm::start(config()).await.expect("dialer");
    let peer = p2p_identity::PeerId::from_bytes([7; 32]);
    dialer.set_observed_addrs(vec![public_quic(45001)]);
    dialer.add_peer_addresses_with_source(peer, vec![public_quic(40000)], AddrSource::Rendezvous);

    let mut events = dialer.subscribe();
    let started = Instant::now();
    let outcome = dialer.connect(peer).await;
    let elapsed = started.elapsed();
    assert!(outcome.is_err(), "hairpin-only peer must fail");
    assert!(
        elapsed < Duration::from_secs(5),
        "hairpin dial must fast-fail within short budget, took {elapsed:?}"
    );
    let mut saw_hairpin_fail = false;
    for _ in 0..4 {
        match p2p_itest::expect_within("DialFailed events", events.recv(), Duration::from_secs(2))
            .await
        {
            Ok(NodeEvent::DialFailed { reason, .. }) if reason.contains("203.0.113.7") => {
                saw_hairpin_fail = true;
                break;
            }
            _ => {}
        }
    }
    assert!(
        saw_hairpin_fail,
        "hairpin failure must be attributable via DialFailed"
    );
}
