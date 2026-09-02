use super::*;
use crate::rendezvous::link::mock::{conn_from_duplex, MockLink};
use crate::rendezvous::messages::{sign_register, unix_now, PeerEntry, FRESH_TOLERANCE_SECS};
use crate::rendezvous::server::{serve_link, RendezvousRegistry};

fn sample_addrs() -> Vec<TransportAddr> {
    vec![TransportAddr::Quic {
        ip: "10.0.0.5".parse().unwrap(),
        port: 4000,
    }]
}

#[test]
fn response_maps_to_peers() {
    let kp = Keypair::generate();
    let addrs = sample_addrs();
    let resp = Response {
        error: String::new(),
        peers: vec![PeerEntry {
            peer_id: kp.peer_id().as_bytes().to_vec(),
            addrs: addrs.iter().map(AddrMsg::from_addr).collect(),
        }],
    };
    let peers = response_to_peers(&resp);
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].0, kp.peer_id());
    assert_eq!(peers[0].1, addrs);
}

#[test]
fn response_skips_bad_entries() {
    let resp = Response {
        error: String::new(),
        peers: vec![PeerEntry {
            peer_id: vec![1, 2, 3],
            addrs: Vec::new(),
        }],
    };
    assert!(response_to_peers(&resp).is_empty());
}

#[tokio::test]
async fn client_registers_and_discovers_other_peer() {
    let (client_side, server_side) = tokio::io::duplex(4096);
    let link: Arc<dyn RendezvousLink> = Arc::new(MockLink::new(client_side));
    let client = RendezvousClient::new(RendezvousConfig::new("room-a", Keypair::generate(), link));

    let registry = Arc::new(RendezvousRegistry::new());
    let server_registry = registry.clone();
    let server_task = tokio::spawn(async move {
        let mut server = conn_from_duplex(server_side);
        let _ = serve_link(&mut server, &server_registry).await;
    });

    // 另一节点直接注册，模拟其已上线
    let other = Keypair::generate();
    let other_addrs = vec![TransportAddr::Quic {
        ip: "10.0.0.9".parse().unwrap(),
        port: 9000,
    }];
    let reg = sign_register(&other, "room-a", &other_addrs, 60, unix_now());
    registry
        .register(&reg, unix_now())
        .expect("other registered");

    let (tx, mut rx) = mpsc::channel(16);
    let cache = MemCache::new();
    let mut conn = client.config.link.connect().await.expect("connect");
    client.register(&mut conn).await.expect("register");
    client
        .query_and_emit(&mut conn, &tx, &cache)
        .await
        .expect("query");

    match rx.recv().await {
        Some(DiscoveryEvent::Discovered(dp)) => {
            assert_eq!(dp.peer, other.peer_id());
            assert_eq!(dp.source, Source::Rendezvous);
            assert_eq!(dp.addrs, other_addrs);
            assert_eq!(cache.get(&other.peer_id()), Some(other_addrs));
        }
        other_ev => panic!("expected Discovered, got {other_ev:?}"),
    }
    server_task.abort();
}

#[tokio::test]
async fn register_timer_fires_periodically() {
    // 回归（E2/E3 链路抖动）：重注册定时器必须周期触发——
    // 初始注册后每 register_interval 再注册一次，兼作控制链路 keepalive
    use crate::rendezvous::messages::request;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let (client_side, server_side) = tokio::io::duplex(4096);
    let link: Arc<dyn RendezvousLink> = Arc::new(MockLink::new(client_side));
    let mut config = RendezvousConfig::new("room-a", Keypair::generate(), link);
    config.addrs = sample_addrs();
    config.ttl_secs = 60;
    config.register_interval = Duration::from_millis(100);
    config.query_interval = Duration::from_secs(3600); // 只统计重注册
    let client = RendezvousClient::new(config);

    let count = Arc::new(AtomicUsize::new(0));
    let count_cl = count.clone();
    let server_task = tokio::spawn(async move {
        let mut server = conn_from_duplex(server_side);
        loop {
            let req = match server.recv_msg::<Request>().await {
                Ok(r) => r,
                Err(_) => break,
            };
            if matches!(req.kind, Some(request::Kind::Register(_))) {
                count_cl.fetch_add(1, Ordering::SeqCst);
            }
            let _ = server.send_msg(&Response::ok()).await;
        }
    });

    let (tx, _rx) = mpsc::channel(16);
    let cache = MemCache::new();
    // 跑 450ms：初始注册 + 每 100ms 重注册；循环被超时打断属预期
    let _ = tokio::time::timeout(
        Duration::from_millis(450),
        client.connect_and_loop(&tx, &cache),
    )
    .await;
    let n = count.load(Ordering::SeqCst);
    assert!(n >= 3, "450ms 内应至少 3 次注册（含初始），实际 {n}");
    server_task.abort();
}

#[test]
fn default_register_interval_within_freshness_and_ttl() {
    // 默认重注册间隔必须：小于注册 TTL（注册无间隙）、
    // 远小于服务端新鲜度容差（±300s）、小于 QUIC idle timeout（30s，兼作 keepalive）
    let (client_side, _server_side) = tokio::io::duplex(64);
    let link: Arc<dyn RendezvousLink> = Arc::new(MockLink::new(client_side));
    let cfg = RendezvousConfig::new("room-a", Keypair::generate(), link);
    assert!(
        cfg.register_interval < Duration::from_secs(cfg.ttl_secs.into()),
        "重注册间隔须小于 TTL"
    );
    assert!(
        cfg.register_interval <= Duration::from_secs(FRESH_TOLERANCE_SECS),
        "重注册间隔须在服务端新鲜度容差内"
    );
    assert!(
        cfg.register_interval < Duration::from_secs(30),
        "重注册间隔须小于 QUIC idle timeout(30s)，否则链路被掐注册有间隙"
    );
}

#[test]
fn backoff_doubles_to_cap_then_resets_after_healthy_session() {
    // E4 回归：退避逐次翻倍封顶 30s；健康会话正常收尾必须复位——
    // 否则长时间在线后一次断连也要等满上限（退避只该惩罚连续失败）
    let mut backoff = ReconnectBackoff::new();
    let seq: Vec<Duration> = (0..8).map(|_| backoff.step()).collect();
    assert_eq!(seq[0], Duration::from_millis(500));
    assert_eq!(seq[1], Duration::from_secs(1));
    assert_eq!(seq[2], Duration::from_secs(2));
    assert_eq!(seq[3], Duration::from_secs(4));
    assert_eq!(seq[4], Duration::from_secs(8));
    assert_eq!(seq[5], Duration::from_secs(16));
    assert_eq!(seq[6], Duration::from_secs(30), "超出上限必须封顶 30s");
    assert_eq!(seq[7], Duration::from_secs(30));
    backoff.reset();
    assert_eq!(backoff.step(), Duration::from_millis(500), "复位后回到初值");
}
