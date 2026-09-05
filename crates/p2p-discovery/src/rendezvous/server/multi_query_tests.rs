//! 多查号回归（BASE1）：服务端同一连接支持多次查号，第二客户端会话照常服务。
//!
//! T23 两机冒烟：第二个借方进程查号挂 10s 握手超时。本文件在 rendezvous
//! 接缝层锚定「多次查号服务」契约；同向重拨的连接级回归见
//! p2p-swarm/tests/base_seams_integration.rs（multi_query_redial_*）。

use std::sync::Arc;

use p2p_identity::{Keypair, PeerId};
use p2p_transport::TransportAddr;

use crate::rendezvous::link::mock::{conn_from_duplex, MockLink};
use crate::rendezvous::link::RendezvousLink;
use crate::rendezvous::messages::{sign_register, unix_now, Request};
use crate::rendezvous::server::{serve_link, RendezvousRegistry};

fn sample_addrs() -> Vec<TransportAddr> {
    vec![TransportAddr::Quic {
        ip: "10.0.0.5".parse().unwrap(),
        port: 4000,
    }]
}

/// 起 serve_link 服务端任务（duplex 半边 + 共享注册表）。
fn spawn_server(server_side: tokio::io::DuplexStream, registry: Arc<RendezvousRegistry>) {
    tokio::spawn(async move {
        let mut server = conn_from_duplex(server_side);
        let _ = serve_link(&mut server, &registry).await;
    });
}

async fn connect(link: Arc<dyn RendezvousLink>) -> crate::rendezvous::RendezvousConn {
    link.connect().await.expect("connect")
}

/// 直接查一个 peer 的地址（query_peer 同构：建连、查询、断开）。
async fn query_peer_on(link: Arc<dyn RendezvousLink>, target: PeerId) -> Vec<TransportAddr> {
    let mut conn = connect(link).await;
    let req = Request::query("base1-mq".into(), target.as_bytes().to_vec());
    let resp = conn.roundtrip(req).await.expect("query roundtrip");
    resp.ensure_ok().expect("query ok");
    crate::rendezvous::client::response_to_peers(&resp)
        .into_iter()
        .find(|(p, _)| *p == target)
        .map(|(_, a)| a)
        .unwrap_or_default()
}

/// 同一连接多次查号：注册 + 连续三轮精确查询，服务端逐次应答。
#[tokio::test]
async fn multi_query_repeated_lookups_on_shared_link() {
    let (client_side, server_side) = tokio::io::duplex(4096);
    let registry = Arc::new(RendezvousRegistry::new());
    spawn_server(server_side, registry.clone());

    let kp = Keypair::generate();
    let link: Arc<dyn RendezvousLink> = Arc::new(MockLink::new(client_side));
    let mut conn = connect(link).await;

    let reg = sign_register(&kp, "base1-mq", &sample_addrs(), 60, unix_now());
    conn.roundtrip(Request::register(reg))
        .await
        .expect("register")
        .ensure_ok()
        .expect("register accepted");

    for round in 0..3u8 {
        let req = Request::query("base1-mq".into(), kp.peer_id().as_bytes().to_vec());
        let resp = conn.roundtrip(req).await.expect("query roundtrip");
        resp.ensure_ok().expect("query ok");
        let found = crate::rendezvous::client::response_to_peers(&resp);
        assert_eq!(found.len(), 1, "round {round}: self entry must come back");
        assert_eq!(found[0].0, kp.peer_id());
    }
}

/// 第二客户端会话照常服务：A 注册并保持连接，B 独立建连查号命中 A，
/// A 的既有连接随后仍可继续查号（服务端会话间互不干扰）。
#[tokio::test]
async fn multi_query_second_session_served_while_first_holds() {
    let registry = Arc::new(RendezvousRegistry::new());

    // 客户端 A：注册并保持连接（模拟常驻借方进程）
    let (a_side, a_server) = tokio::io::duplex(4096);
    spawn_server(a_server, registry.clone());
    let kp_a = Keypair::generate();
    let link_a: Arc<dyn RendezvousLink> = Arc::new(MockLink::new(a_side));
    let mut conn_a = connect(link_a).await;
    let reg = sign_register(&kp_a, "base1-mq", &sample_addrs(), 60, unix_now());
    conn_a
        .roundtrip(Request::register(reg))
        .await
        .expect("A register")
        .ensure_ok()
        .expect("A register accepted");

    // 客户端 B：独立会话查号 A（第二进程形态）
    let (b_side, b_server) = tokio::io::duplex(4096);
    spawn_server(b_server, registry.clone());
    let link_b: Arc<dyn RendezvousLink> = Arc::new(MockLink::new(b_side));
    let hit = query_peer_on(link_b, kp_a.peer_id()).await;
    assert_eq!(hit, sample_addrs(), "B must resolve A via the bootstrap");

    // A 的既有连接继续查号：两次会话互不干扰
    let req = Request::query("base1-mq".into(), kp_a.peer_id().as_bytes().to_vec());
    let resp = conn_a.roundtrip(req).await.expect("A later query");
    resp.ensure_ok().expect("A later query ok");
    assert_eq!(resp.peers.len(), 1);
}
