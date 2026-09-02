use super::*;
use crate::DialHop;
use std::time::Duration;

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
