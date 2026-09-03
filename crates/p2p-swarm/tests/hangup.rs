//! 挂断路径集成测试（GUI peer_disconnect 的底座公开 API 面）。

use std::{sync::Arc, time::Duration};

use p2p_identity::Keypair;
use p2p_protocol::HandlerRegistry;
use p2p_swarm::{NodeEvent, Swarm, SwarmConfig};
use p2p_transport::TransportAddr;

fn test_config() -> SwarmConfig {
    SwarmConfig {
        keypair: Arc::new(Keypair::generate()),
        quic_port: 0,
        tcp_port: 0,
        registry: Arc::new(HandlerRegistry::default()),
        relay_addrs: Vec::new(),
        advertised_addrs: Vec::new(),
    }
}

/// 挂断：出池 + 关连接，双方各发一次 PeerDisconnected；重复挂断幂等返回 false。
#[tokio::test]
async fn disconnect_closes_conn_and_emits_once_per_side() {
    let swarm = Swarm::start(test_config()).await.expect("bind swarm");
    let helper = Swarm::start(test_config()).await.expect("bind helper");
    let peer = helper.local_peer_id();
    let addr = helper
        .listen_addrs()
        .into_iter()
        .find(|a| matches!(a, TransportAddr::Tcp { .. }))
        .expect("helper tcp addr");
    swarm.add_peer_addresses(peer, vec![addr]);
    swarm.connect(peer).await.expect("dial helper");

    let mut events = swarm.subscribe();
    let mut helper_events = helper.subscribe();
    assert!(swarm.disconnect(&peer), "in-register conn must hang up");
    assert!(!swarm.disconnect(&peer), "repeat hangup must be idempotent");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let (mut local_bye, mut remote_bye) = (false, false);
    while (!local_bye || !remote_bye) && tokio::time::Instant::now() < deadline {
        tokio::select! {
            ev = events.recv() => local_bye |= matches!(ev, Ok(NodeEvent::PeerDisconnected { .. })),
            ev = helper_events.recv() => remote_bye |= matches!(ev, Ok(NodeEvent::PeerDisconnected { .. })),
        }
    }
    assert!(local_bye, "local side must emit PeerDisconnected");
    assert!(remote_bye, "helper side must observe the close");
    assert_eq!(swarm.metrics().active_connections, 0, "pool must be empty");
}
