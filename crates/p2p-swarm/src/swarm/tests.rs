use super::*;
use crate::DialHop;
use std::time::Duration;

fn tcp(ip: &str, port: u16) -> TransportAddr {
    TransportAddr::Tcp {
        ip: ip.parse().expect("valid ip"),
        port,
    }
}

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

/// 直连按地址顺序尝试：首地址拨号失败必须换下一地址成功，
/// 且失败地址发 DialFailed（design §12 失败路径可见）。
#[tokio::test]
async fn dial_falls_through_failed_addr() {
    let swarm = Swarm::start(test_config()).await.expect("bind swarm");
    let helper = Swarm::start(test_config()).await.expect("bind helper");

    let helper_peer = helper.local_peer_id();
    let tcp_addr = helper
        .listen_addrs()
        .into_iter()
        .find(|a| matches!(a, TransportAddr::Tcp { .. }))
        .expect("helper tcp addr");
    // 首地址用本机未监听的 TCP 端口：loopback 拒绝即时返回（UDP 拒绝在 macOS 上要等 30s 超时）
    swarm.add_peer_addresses(
        helper_peer,
        vec![
            TransportAddr::Tcp {
                ip: IpAddr::from([127, 0, 0, 1]),
                port: 1,
            },
            tcp_addr,
        ],
    );

    let mut events = swarm.subscribe();
    swarm.connect(helper_peer).await.expect("dial via fallback");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut saw_failed = false;
    loop {
        match tokio::time::timeout_at(deadline, events.recv()).await {
            Ok(Ok(NodeEvent::DialFailed { .. })) => {
                saw_failed = true;
                break;
            }
            Ok(Ok(_)) => continue,
            _ => break,
        }
    }
    assert!(saw_failed, "failed first addr must emit DialFailed");
}

/// 未配置 relay 时降级链止于直连：Relay 跳不可用要有日志，且不误发 Relay 成功事件。
#[tokio::test]
async fn degrade_without_relay_reports_unavailable() {
    let swarm = Swarm::start(test_config()).await.expect("bind swarm");
    let peer = PeerId::from_bytes([9; 32]);
    swarm.add_peer_addresses(
        peer,
        vec![TransportAddr::Tcp {
            ip: IpAddr::from([127, 0, 0, 1]),
            port: 1,
        }],
    );

    let mut events = swarm.subscribe();
    let outcome = swarm.connect(peer).await;
    assert!(outcome.is_err(), "unreachable peer must fail without relay");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut saw_direct_fail = false;
    let mut saw_relay_ok = false;
    loop {
        match tokio::time::timeout_at(deadline, events.recv()).await {
            Ok(Ok(NodeEvent::DialHop {
                hop: DialHop::Direct,
                ok: false,
                ..
            })) => {
                saw_direct_fail = true;
            }
            Ok(Ok(NodeEvent::DialHop {
                hop: DialHop::Relay,
                ok: true,
                ..
            })) => {
                saw_relay_ok = true;
            }
            Ok(Ok(NodeEvent::DialFailed { .. })) => {}
            Ok(Ok(NodeEvent::PeerDiscovered { .. })) => {}
            _ => break,
        }
    }
    assert!(saw_direct_fail, "direct hop failure must be visible");
    assert!(!saw_relay_ok, "relay hop must not fake success");
}

/// E3 回归：观测到全局地址时，打洞/注册宣告过滤 loopback（对端不可拨）。
#[tokio::test]
async fn punch_addrs_drop_loopback_when_global_observed() {
    let swarm = Swarm::start(test_config()).await.expect("bind swarm");
    swarm.set_observed_addrs(vec![TransportAddr::Quic {
        ip: "203.0.113.7".parse().unwrap(),
        port: 45001,
    }]);
    let addrs = swarm.punch_addrs_strs();
    assert!(
        addrs.iter().all(|s| !s.contains("127.0.0.1")),
        "loopback must be filtered when global observed exists: {addrs:?}"
    );
    assert!(
        addrs.iter().any(|s| s.contains("203.0.113.7")),
        "global observed must stay"
    );
}

/// 混合地址回归（E3 小修单）：QUIC 地址必拒 + TCP 地址可达——
/// 直连跳必须遍历全部地址（QUIC 按序先试、失败发 DialFailed），经 TCP 成功，
/// 不得在首个地址被拒后直接上抛。
#[tokio::test]
async fn dial_traverses_mixed_quic_and_tcp_addrs() {
    let swarm = Swarm::start(test_config()).await.expect("bind swarm");
    let helper = Swarm::start(test_config()).await.expect("bind helper");
    let helper_peer = helper.local_peer_id();
    let tcp_addr = helper
        .listen_addrs()
        .into_iter()
        .find(|a| matches!(a, TransportAddr::Tcp { .. }))
        .expect("helper tcp addr");
    // 端口 0 的 QUIC 地址：quinn 拨号即报错（必拒且立即返回）
    swarm.add_peer_addresses(
        helper_peer,
        vec![
            TransportAddr::Quic {
                ip: IpAddr::from([127, 0, 0, 1]),
                port: 0,
            },
            tcp_addr,
        ],
    );

    let mut events = swarm.subscribe();
    swarm
        .connect(helper_peer)
        .await
        .expect("dial must land on the reachable tcp addr");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut saw_quic_fail = false;
    loop {
        match tokio::time::timeout_at(deadline, events.recv()).await {
            Ok(Ok(NodeEvent::DialFailed { reason, .. })) if reason.contains("/u0") => {
                saw_quic_fail = true;
                break;
            }
            Ok(Ok(_)) => continue,
            _ => break,
        }
    }
    assert!(
        saw_quic_fail,
        "refused quic addr must emit DialFailed before tcp success"
    );
}

/// E3 排序回归（来源透传）：mDNS 地址与全局观测地址混合时，
/// mDNS/同网段地址先试（失败即换），全局地址殿后；可达的同网段地址落地。
/// 全部地址都用立即失败端口（未监听 loopback / 端口 0），避免长超时。
#[tokio::test]
async fn dial_prefers_mdns_and_lan_addrs_over_global() {
    let swarm = Swarm::start(test_config()).await.expect("bind swarm");
    let helper = Swarm::start(test_config()).await.expect("bind helper");
    let helper_peer = helper.local_peer_id();
    let tcp_addr = helper
        .listen_addrs()
        .into_iter()
        .find(|a| matches!(a, TransportAddr::Tcp { .. }))
        .expect("helper tcp addr");
    // 可达的同网段地址：观测 IP + helper 的 TCP 端口（macOS lo0 仅配 127.0.0.1）
    let lan_reachable = tcp_addr.clone();

    // 登记顺序故意乱序：全局观测(端口 0 必拒) → mDNS(未监听端口必拒) → 同网段可达
    swarm.add_peer_addresses_with_source(
        helper_peer,
        vec![tcp("10.99.99.99", 0)],
        AddrSource::Rendezvous,
    );
    swarm.add_peer_addresses_with_source(helper_peer, vec![tcp("127.0.0.1", 1)], AddrSource::Mdns);
    swarm.add_peer_addresses_with_source(helper_peer, vec![lan_reachable], AddrSource::Rendezvous);

    let mut events = swarm.subscribe();
    swarm
        .connect(helper_peer)
        .await
        .expect("dial must land on the lan-subnet addr");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut failed = Vec::new();
    loop {
        match tokio::time::timeout_at(deadline, events.recv()).await {
            Ok(Ok(NodeEvent::DialFailed { reason, .. })) => failed.push(reason),
            Ok(Ok(_)) => continue,
            _ => break,
        }
    }
    // 排序断言：mDNS 地址最先试（先失败），同网段可达地址随后成功；
    // 全局地址（端口 0）排在可达地址之后，根本未被尝试。
    assert!(
        failed.iter().any(|r| r.contains("127.0.0.1/t1")),
        "mdns addr must be tried first, got {failed:?}"
    );
    assert!(
        !failed.iter().any(|r| r.contains("10.99.99.99")),
        "global addr must rank after the reachable lan addr, got {failed:?}"
    );
}
