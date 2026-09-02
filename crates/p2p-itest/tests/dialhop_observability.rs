//! E4: 验证 TCP 拒绝后仍遍历后续地址，并断言连接事件可见。

use std::net::IpAddr;
use std::sync::Arc;

use p2p_identity::Keypair;
use p2p_protocol::HandlerRegistry;
use p2p_swarm::{NodeEvent, Swarm, SwarmConfig};
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
async fn refused_tcp_continues_to_reachable_address() {
    let dialer = Swarm::start(config()).await.expect("dialer");
    let target = Swarm::start(config()).await.expect("target");
    let peer = target.local_peer_id();
    let reachable = target
        .listen_addrs()
        .into_iter()
        .find(|a| matches!(a, TransportAddr::Tcp { .. }))
        .expect("target tcp addr");
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
    let mut events = dialer.subscribe();

    dialer
        .connect(peer)
        .await
        .expect("must continue after refusal");

    let mut saw_refused = false;
    let mut saw_connected = false;
    for _ in 0..4 {
        match p2p_itest::expect_within(
            "DialHop events",
            events.recv(),
            std::time::Duration::from_secs(2),
        )
        .await
        {
            Ok(NodeEvent::DialFailed { reason, .. }) if reason.contains("/t1") => {
                saw_refused = true
            }
            Ok(NodeEvent::PeerConnected { peer: got }) if got == peer => {
                saw_connected = true;
                break;
            }
            Ok(_) => {}
            Err(_) => {}
        }
    }
    assert!(saw_refused, "refused TCP address must be observable");
    // 直连第二地址成功时不发 Direct=false；该事件仅表示整跳耗尽。
    assert!(saw_connected, "later reachable address must connect");
}
