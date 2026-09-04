//! RS 排障回归：生产盲拨客户端（TransportLink）↔ facade bootstrap 节点。
//!
//! 实验室跨进程实测（2026-09-04，docs/ops/repair-p0b-drill.md 同构环境）发现：
//! helper 经 rendezvous 接线后 register roundtrip 无应答，且 facade liveness
//! probe（10s×3 次）未获应答后约 33s 掐线，客户端卡死死连接不重拨。本文件
//! 用真实 loopback socket 锚定两条契约：
//! A) register/query roundtrip 在 facade 服务端装配下可用（QUIC/TCP）；
//! B) 探活窗口过后同一盲拨连接仍可用——客户端侧必须应答 /p2p-base/ping/1。

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use p2p::TransportLink;
use p2p_discovery::rendezvous::messages::{sign_register, unix_now, Request};
use p2p_discovery::RendezvousLink;
use p2p_identity::Keypair;
use p2p_itest::expect_within;
use p2p_transport::TransportAddr;

const NS: &str = "rs-link";
const LIMIT: Duration = Duration::from_secs(30);
/// 探活窗口：probe_interval 10s × max_probe_misses 3 + 超时余量。
const PROBE_WINDOW: Duration = Duration::from_secs(37);

fn temp_root(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rf-link-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// facade bootstrap 节点（rendezvous 服务端角色，随机端口，mDNS 关）。
async fn spawn_facade(tag: &str) -> p2p::Node {
    let node = p2p::Node::builder()
        .data_dir(temp_root(tag))
        .mdns(false)
        .rendezvous_public_only(false)
        .build()
        .await
        .expect("facade bootstrap builds");
    assert!(!node.listen_addrs().is_empty(), "listen addrs advertised");
    node
}

/// "ip/uPORT" / "ip/tPORT" → TransportAddr（listen_addrs 展示格式）。
fn parse_listen(entry: &str) -> TransportAddr {
    let (ip_str, tail) = entry.split_once('/').expect("addr split");
    let ip: IpAddr = ip_str.parse().expect("ip parses");
    let mut chars = tail.chars();
    let kind = chars.next().expect("kind char");
    let port: u16 = chars.as_str().parse().expect("port parses");
    match kind {
        'u' => TransportAddr::Quic { ip, port },
        't' => TransportAddr::Tcp { ip, port },
        _ => panic!("unknown transport kind in {entry}"),
    }
}

fn pick(addrs: &[String], kind: char) -> TransportAddr {
    let entry = addrs
        .iter()
        .find(|a| a.contains(&format!("/{}", kind)))
        .or_else(|| addrs.first())
        .expect("listen addr present");
    parse_listen(entry)
}

fn advertised_routable() -> Vec<TransportAddr> {
    vec![TransportAddr::Quic {
        ip: "10.0.0.1".parse().unwrap(),
        port: 7001,
    }]
}

fn register_req(kp: &Keypair) -> Request {
    let reg = sign_register(kp, NS, &advertised_routable(), 120, unix_now());
    Request::register(reg)
}

async fn register_query_roundtrip(
    conn: &mut p2p_discovery::rendezvous::RendezvousConn,
    kp: &Keypair,
) {
    let resp = expect_within(
        "register roundtrip",
        conn.roundtrip(register_req(kp)),
        LIMIT,
    )
    .await
    .expect("register io");
    resp.ensure_ok().expect("register accepted");

    let q = Request::query(NS.into(), kp.peer_id().as_bytes().to_vec());
    let resp = expect_within("self query roundtrip", conn.roundtrip(q), LIMIT)
        .await
        .expect("query io");
    resp.ensure_ok().expect("query ok");
    assert_eq!(resp.peers.len(), 1, "self entry must come back");
    assert_eq!(resp.peers[0].peer_id, kp.peer_id().as_bytes().to_vec());
}

#[tokio::test]
async fn quic_blind_dial_register_query_roundtrip() {
    let node = spawn_facade("quic").await;
    let addr = pick(&node.listen_addrs(), 'u');
    let link = TransportLink::new(vec![addr], Arc::new(Keypair::generate())).expect("link builds");
    let mut conn = expect_within("blind dial", link.connect(), LIMIT)
        .await
        .expect("blind dial connects");
    let kp = Keypair::generate();
    register_query_roundtrip(&mut conn, &kp).await;
    node.shutdown();
}

#[tokio::test]
async fn tcp_blind_dial_register_query_roundtrip() {
    let node = spawn_facade("tcp").await;
    let addr = pick(&node.listen_addrs(), 't');
    let link = TransportLink::new(vec![addr], Arc::new(Keypair::generate())).expect("link builds");
    let mut conn = expect_within("blind dial", link.connect(), LIMIT)
        .await
        .expect("blind dial connects");
    let kp = Keypair::generate();
    register_query_roundtrip(&mut conn, &kp).await;
    node.shutdown();
}

/// 探活窗口生存：facade 对盲拨连接发起 liveness probe（/p2p-base/ping/1），
/// 客户端必须应答，否则连接在约 33s 被判死掐线（实验室实测路径）。
#[tokio::test]
async fn quic_blind_dial_conn_survives_probe_window() {
    let node = spawn_facade("probe").await;
    let addr = pick(&node.listen_addrs(), 'u');
    let link = TransportLink::new(vec![addr], Arc::new(Keypair::generate())).expect("link builds");
    let mut conn = expect_within("blind dial", link.connect(), LIMIT)
        .await
        .expect("blind dial connects");
    let kp = Keypair::generate();
    let resp = expect_within("first register", conn.roundtrip(register_req(&kp)), LIMIT)
        .await
        .expect("first register io");
    resp.ensure_ok().expect("first register accepted");

    tokio::time::sleep(PROBE_WINDOW).await;

    let reg = sign_register(&kp, NS, &advertised_routable(), 120, unix_now());
    let resp = expect_within(
        "post-probe register",
        conn.roundtrip(Request::register(reg)),
        LIMIT,
    )
    .await
    .expect("connection must stay usable across probe window");
    resp.ensure_ok().expect("post-probe register accepted");
    node.shutdown();
}
