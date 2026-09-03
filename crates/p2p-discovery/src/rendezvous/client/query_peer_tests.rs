//! query_peer 回归：按 PeerId 精确查询、未知对端返回空、strip 过滤生效。

use super::*;
use crate::rendezvous::link::mock::{conn_from_duplex, MockLink};
use crate::rendezvous::messages::{sign_register, unix_now};
use crate::rendezvous::server::{serve_link, RendezvousRegistry};

fn quic(ip: &str, port: u16) -> TransportAddr {
    TransportAddr::Quic {
        ip: ip.parse().expect("valid ip"),
        port,
    }
}

fn known_peer() -> Keypair {
    Keypair::generate()
}

async fn registry_with_peer(peer: &Keypair, addrs: &[TransportAddr]) -> Arc<RendezvousRegistry> {
    let registry = Arc::new(RendezvousRegistry::new());
    let reg = sign_register(peer, "room-a", addrs, 60, unix_now());
    registry.register(&reg, unix_now()).expect("register ok");
    registry
}

fn spawn_server(registry: Arc<RendezvousRegistry>, server_side: tokio::io::DuplexStream) {
    tokio::spawn(async move {
        let mut server = conn_from_duplex(server_side);
        let _ = serve_link(&mut server, &registry).await;
    });
}

#[tokio::test]
async fn query_peer_returns_known_peer_addrs() {
    let peer = known_peer();
    let addrs = vec![quic("10.0.0.9", 9000)];
    let (client_side, server_side) = tokio::io::duplex(4096);
    let link: Arc<dyn RendezvousLink> = Arc::new(MockLink::new(client_side));
    let client = RendezvousClient::new(RendezvousConfig::new("room-a", Keypair::generate(), link));
    let registry = registry_with_peer(&peer, &addrs).await;
    spawn_server(registry, server_side);

    let got = client.query_peer(peer.peer_id()).await.expect("query ok");
    assert_eq!(got, addrs, "精确查询必须返回该对端的登记地址");
}

#[tokio::test]
async fn query_peer_unknown_peer_returns_empty() {
    let (client_side, server_side) = tokio::io::duplex(4096);
    let link: Arc<dyn RendezvousLink> = Arc::new(MockLink::new(client_side));
    let client = RendezvousClient::new(RendezvousConfig::new("room-a", Keypair::generate(), link));
    spawn_server(Arc::new(RendezvousRegistry::new()), server_side);

    let got = client
        .query_peer(Keypair::generate().peer_id())
        .await
        .expect("query ok");
    assert!(got.is_empty(), "未注册对端必须返回空而不是错误");
}

#[tokio::test]
async fn query_peer_strips_unroutable_when_enabled() {
    let peer = known_peer();
    let addrs = vec![quic("127.0.0.1", 40000), quic("10.0.0.9", 9000)];
    let (client_side, server_side) = tokio::io::duplex(4096);
    let link: Arc<dyn RendezvousLink> = Arc::new(MockLink::new(client_side));
    let client = RendezvousClient::new(RendezvousConfig::new("room-a", Keypair::generate(), link));
    let registry = registry_with_peer(&peer, &addrs).await;
    spawn_server(registry, server_side);

    let got = client.query_peer(peer.peer_id()).await.expect("query ok");
    assert_eq!(
        got,
        vec![quic("10.0.0.9", 9000)],
        "strip_unroutable 默认开启：loopback 地址必须剥离"
    );
}
