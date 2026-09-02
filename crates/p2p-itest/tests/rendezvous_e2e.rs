//! 互操作：p2p-discovery rendezvous 客户端 ↔ 服务端经 duplex 全流程：
//! 签名注册、查询返回地址、篡改签名被显式拒绝。

use std::sync::Arc;
use std::time::Duration;

use p2p_discovery::rendezvous::messages::{sign_register, verify_register, Request};
use p2p_discovery::rendezvous::server::{serve_link, RendezvousRegistry};
use p2p_discovery::{Discovery, DiscoveryEvent, RendezvousClient, RendezvousConfig, Source};
use p2p_identity::Keypair;
use p2p_transport::TransportAddr;
use tokio::sync::{mpsc, Mutex};

use p2p_itest::{expect_within, rendezvous_conn, SingleDuplexLink};

const NAMESPACE: &str = "itest-room";
const LIMIT: Duration = Duration::from_secs(10);

fn quic_addr(ip: &str, port: u16) -> TransportAddr {
    TransportAddr::Quic { ip: ip.parse().expect("valid ip"), port }
}

/// 在服务侧 duplex 半边起 serve_link，与注册表共享。
fn spawn_server(
    registry: Arc<RendezvousRegistry>,
    server_io: tokio::io::DuplexStream,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut conn = rendezvous_conn(server_io);
        let _ = serve_link(&mut conn, &registry).await;
    })
}

/// 起一个真实 RendezvousClient（独立 server 连接），周期注册/查询。
fn spawn_client(
    registry: Arc<RendezvousRegistry>,
    kp: Keypair,
    addrs: Vec<TransportAddr>,
) -> mpsc::Receiver<DiscoveryEvent> {
    let (io, server_io) = tokio::io::duplex(4096);
    let _server = spawn_server(registry, server_io);
    let link: Arc<dyn p2p_discovery::rendezvous::RendezvousLink> =
        Arc::new(SingleDuplexLink(Mutex::new(Some(io))));
    let mut config = RendezvousConfig::new(NAMESPACE, kp, link);
    config.addrs = addrs;
    config.ttl_secs = 60;
    config.register_interval = Duration::from_millis(200);
    config.query_interval = Duration::from_millis(200);
    let client = Arc::new(RendezvousClient::new(config));
    let (tx, rx) = mpsc::channel(16);
    tokio::spawn(client.run(tx));
    rx
}

#[tokio::test]
async fn client_discovers_registered_peer_over_duplex() {
    let registry = Arc::new(RendezvousRegistry::new());

    let kp_a = Keypair::generate();
    let addrs_a = vec![quic_addr("10.0.0.1", 7001)];
    let _rx_a = spawn_client(registry.clone(), kp_a.clone(), addrs_a.clone());

    let kp_b = Keypair::generate();
    let mut rx_b = spawn_client(registry, kp_b, Vec::new());

    let discovered = expect_within(
        "discovery of peer A",
        async {
            match rx_b.recv().await {
                Some(DiscoveryEvent::Discovered(dp)) => dp,
                Some(DiscoveryEvent::Failed { source, reason }) => {
                    panic!("discovery must not fail on healthy path: {source:?} {reason}")
                }
                Some(other) => panic!("unexpected event {other:?}"),
                None => panic!("event channel closed before discovery"),
            }
        },
        LIMIT,
    )
    .await;

    assert_eq!(discovered.peer, kp_a.peer_id(), "must discover A by identity");
    assert_eq!(discovered.addrs, addrs_a, "returned addrs must match registration");
    assert_eq!(discovered.source, Source::Rendezvous);
    assert!(discovered.expires_at.is_some(), "rendezvous entries carry TTL");
}

#[tokio::test]
async fn tampered_signature_rejected_with_explicit_error() {
    let registry = Arc::new(RendezvousRegistry::new());
    let (io, server_io) = tokio::io::duplex(4096);
    let _server = spawn_server(registry, server_io);
    let mut conn = rendezvous_conn(io);

    let kp = Keypair::generate();
    let mut reg = sign_register(&kp, NAMESPACE, &[quic_addr("10.0.0.9", 9000)], 60);
    reg.addrs[0].port = 9999;
    assert!(!verify_register(&reg), "tamper must invalidate signature locally");

    let resp = expect_within("register roundtrip", conn.roundtrip(Request::register(reg)), LIMIT)
        .await
        .expect("roundtrip io");
    let err = resp
        .ensure_ok()
        .expect_err("tampered register must be rejected with explicit error");
    assert_eq!(err, "bad signature", "server must name the rejection reason");
}

#[tokio::test]
async fn query_unknown_peer_returns_empty_but_valid_response() {
    let registry = Arc::new(RendezvousRegistry::new());
    let (io, server_io) = tokio::io::duplex(4096);
    let _server = spawn_server(registry, server_io);
    let mut conn = rendezvous_conn(io);

    let unknown = Keypair::generate();
    let resp = expect_within(
        "query roundtrip",
        conn.roundtrip(Request::query(
            NAMESPACE.into(),
            unknown.peer_id().as_bytes().to_vec(),
        )),
        LIMIT,
    )
    .await
    .expect("roundtrip io");
    assert!(resp.ensure_ok().is_ok(), "unknown peer is empty, not an error");
    assert!(resp.peers.is_empty(), "no phantom entries allowed");
}
