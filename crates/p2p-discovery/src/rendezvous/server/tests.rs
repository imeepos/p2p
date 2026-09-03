use super::*;
use p2p_identity::Keypair;
use std::sync::Arc;

use crate::rendezvous::link::mock::conn_from_duplex;
use crate::rendezvous::messages::{sign_register, unix_now, FRESH_TOLERANCE_SECS};

fn sample_addrs() -> Vec<TransportAddr> {
    vec![TransportAddr::Quic {
        ip: "10.0.0.5".parse().unwrap(),
        port: 4000,
    }]
}

fn valid_reg(namespace: &str) -> (Keypair, Register) {
    let kp = Keypair::generate();
    let reg = sign_register(&kp, namespace, &sample_addrs(), 60, unix_now());
    (kp, reg)
}

#[test]
fn registry_rejects_tampered_register() {
    let (_, mut reg) = valid_reg("room-a");
    let now = unix_now();
    reg.addrs[0].port = 9999;
    let registry = RendezvousRegistry::new();
    assert!(registry.register(&reg, now).is_err());
}

#[test]
fn registry_rejects_wrong_peer_id() {
    let (_, mut reg) = valid_reg("room-a");
    let other = Keypair::generate();
    let now = unix_now();
    reg.peer_id = other.peer_id().as_bytes().to_vec();
    let registry = RendezvousRegistry::new();
    assert!(registry.register(&reg, now).is_err());
}

#[test]
fn registry_accepts_valid_register_and_query() {
    let (_, reg) = valid_reg("room-a");
    let now = unix_now();
    let registry = RendezvousRegistry::new();
    registry.register(&reg, now).expect("register ok");
    let resp = registry.query(&Query {
        namespace: "room-a".into(),
        peer_id: reg.peer_id.clone(),
    });
    assert!(resp.error.is_empty());
    assert_eq!(resp.peers.len(), 1);
    assert_eq!(resp.peers[0].peer_id, reg.peer_id);
}

#[test]
fn tampered_ttl_replay_rejected() {
    // 必测 1：TTL 被篡改的重放帧被拒（签名覆盖 ttl，篡改即验签失败）
    let (_, mut reg) = valid_reg("room-a");
    let now = unix_now();
    reg.ttl_secs = 999_999;
    let registry = RendezvousRegistry::new();
    assert!(registry.register(&reg, now).is_err());
}

#[test]
fn stale_signature_rejected() {
    // 必测 2：过期签名被拒（签名有效但超出重放窗口）
    let kp = Keypair::generate();
    let now = unix_now();
    let stale = sign_register(
        &kp,
        "room-a",
        &sample_addrs(),
        60,
        now - FRESH_TOLERANCE_SECS - 100,
    );
    let registry = RendezvousRegistry::new();
    assert!(registry.register(&stale, now).is_err());
}

#[test]
fn empty_namespace_rejected() {
    let (_, reg) = valid_reg("");
    let now = unix_now();
    let registry = RendezvousRegistry::new();
    assert!(registry.register(&reg, now).is_err());
}

#[test]
fn oversized_namespace_rejected() {
    let long_ns = "n".repeat(MAX_NAMESPACE_LEN + 1);
    let (_, reg) = valid_reg(&long_ns);
    let now = unix_now();
    let registry = RendezvousRegistry::new();
    assert!(registry.register(&reg, now).is_err());
}

#[test]
fn huge_ttl_capped_acceptance() {
    // TTL 超上限被截断而非长占；注册仍成功且可查询
    let kp = Keypair::generate();
    let now = unix_now();
    let reg = sign_register(&kp, "room-a", &sample_addrs(), u32::MAX, now);
    let registry = RendezvousRegistry::new();
    registry.register(&reg, now).expect("register ok");
    let resp = registry.query(&Query {
        namespace: "room-a".into(),
        peer_id: reg.peer_id.clone(),
    });
    assert_eq!(resp.peers.len(), 1);
}

#[test]
fn namespace_peer_cap_enforced() {
    // 第 MAX_PEERS_PER_NAMESPACE+1 个不同 peer 注册被拒
    let registry = RendezvousRegistry::new();
    let now = unix_now();
    for _ in 0..MAX_PEERS_PER_NAMESPACE {
        let (_, reg) = valid_reg("room-a");
        registry.register(&reg, now).expect("register ok");
    }
    let (_, overflow) = valid_reg("room-a");
    assert!(registry.register(&overflow, now).is_err());
}

#[tokio::test]
async fn full_duplex_client_server_roundtrip() {
    let (client_side, server_side) = tokio::io::duplex(4096);
    let mut client = conn_from_duplex(client_side);
    let registry = Arc::new(RendezvousRegistry::new());
    let server_registry = registry.clone();
    let server_task = tokio::spawn(async move {
        let mut server = conn_from_duplex(server_side);
        let _ = serve_link(&mut server, &server_registry).await;
    });

    let (_, reg) = valid_reg("room-a");
    let resp = client
        .roundtrip(Request::register(reg.clone()))
        .await
        .expect("register roundtrip");
    resp.ensure_ok().expect("register accepted");

    let resp = client
        .roundtrip(Request::query("room-a".into(), reg.peer_id.clone()))
        .await
        .expect("query roundtrip");
    assert!(resp.error.is_empty());
    assert_eq!(resp.peers.len(), 1);
    assert_eq!(resp.peers[0].peer_id, reg.peer_id);
    server_task.abort();
}

#[tokio::test]
async fn bad_signature_gets_error_response() {
    let (client_side, server_side) = tokio::io::duplex(4096);
    let mut client = conn_from_duplex(client_side);
    let registry = Arc::new(RendezvousRegistry::new());
    let server_registry = registry.clone();
    let server_task = tokio::spawn(async move {
        let mut server = conn_from_duplex(server_side);
        let _ = serve_link(&mut server, &server_registry).await;
    });

    let (_, mut reg) = valid_reg("room-a");
    reg.addrs[0].port = 9999; // 篡改后签名不匹配
    let resp = client
        .roundtrip(Request::register(reg))
        .await
        .expect("roundtrip");
    assert!(resp.ensure_ok().is_err());
    server_task.abort();
}

#[tokio::test]
async fn register_rate_limit_enforced() {
    // 每连接令牌桶：满桶容量 10，第 11 次注册应被限速拒绝
    let (client_side, server_side) = tokio::io::duplex(4096);
    let mut client = conn_from_duplex(client_side);
    let registry = Arc::new(RendezvousRegistry::new());
    let server_registry = registry.clone();
    let server_task = tokio::spawn(async move {
        let mut server = conn_from_duplex(server_side);
        let _ = serve_link(&mut server, &server_registry).await;
    });

    let (_, reg) = valid_reg("room-a");
    let mut last_err = None;
    for _ in 0..=(RATE_LIMIT_PER_MINUTE as usize) {
        let resp = client
            .roundtrip(Request::register(reg.clone()))
            .await
            .expect("roundtrip");
        if let Err(e) = resp.ensure_ok() {
            last_err = Some(e);
        }
    }
    assert!(
        last_err.is_some(),
        "第 {} 次注册应被限速拒绝",
        RATE_LIMIT_PER_MINUTE + 1
    );
    server_task.abort();
}

#[tokio::test]
async fn query_unknown_peer_returns_empty() {
    let registry = RendezvousRegistry::new();
    let kp = Keypair::generate();
    let unknown = kp.peer_id().as_bytes().to_vec();
    let resp = registry.query(&Query {
        namespace: "room-a".into(),
        peer_id: unknown,
    });
    assert!(resp.error.is_empty());
    assert!(resp.peers.is_empty());
}

#[test]
fn query_unknown_namespace_does_not_grow_registry() {
    // 审计 HIGH 回归：query 只读，未知 namespace 不得创建缓存条目
    let registry = RendezvousRegistry::new();
    for ns in ["ghost-room", "", &"x".repeat(MAX_NAMESPACE_LEN + 1)] {
        let resp = registry.query(&Query {
            namespace: ns.to_string(),
            peer_id: Vec::new(),
        });
        assert!(resp.error.is_empty());
        assert!(resp.peers.is_empty());
    }
    assert!(
        registry.namespaces.lock().unwrap().is_empty(),
        "query must not create namespace entries"
    );
}

#[test]
fn oversized_addr_list_rejected() {
    // 审查 M8：单条注册地址数超上限被拒（签名仍有效，防大帧撑资源）
    let kp = Keypair::generate();
    let many: Vec<TransportAddr> = (0..=MAX_ADDRS_PER_REGISTER)
        .map(|i| TransportAddr::Quic {
            ip: "10.0.0.5".parse().unwrap(),
            port: 4000 + i as u16,
        })
        .collect();
    let reg = sign_register(&kp, "room-a", &many, 60, unix_now());
    let registry = RendezvousRegistry::new();
    let err = registry.register(&reg, unix_now()).expect_err("over cap");
    assert!(err.contains("addr count"), "got {err}");
}

#[tokio::test]
async fn query_rate_limit_enforced() {
    // 审查 M8：查询同样受每连接令牌桶约束（满桶 120，第 122 次被拒）
    let (client_side, server_side) = tokio::io::duplex(4096);
    let mut client = conn_from_duplex(client_side);
    let registry = Arc::new(RendezvousRegistry::new());
    let server_registry = registry.clone();
    let server_task = tokio::spawn(async move {
        let mut server = conn_from_duplex(server_side);
        let _ = serve_link(&mut server, &server_registry).await;
    });
    let mut last_err = None;
    for _ in 0..=(RATE_LIMIT_QUERIES_PER_MINUTE as usize) {
        let resp = client
            .roundtrip(Request::query("ghost-room".into(), Vec::new()))
            .await
            .expect("roundtrip");
        if let Err(e) = resp.ensure_ok() {
            last_err = Some(e);
        }
    }
    assert!(
        last_err.is_some(),
        "第 {} 次查询应被限速拒绝",
        RATE_LIMIT_QUERIES_PER_MINUTE + 1
    );
    server_task.abort();
}

fn loopback_reg(namespace: &str) -> Register {
    let kp = Keypair::generate();
    let addrs = vec![TransportAddr::Quic {
        ip: "127.0.0.1".parse().unwrap(),
        port: 40000,
    }];
    sign_register(&kp, namespace, &addrs, 60, unix_now())
}

#[test]
fn public_only_rejects_all_unroutable_register() {
    // E5 回归：公共 rendezvous 拒收全 loopback 注册（观测失败节点的 localhost 泄漏口）
    let reg = loopback_reg("room-a");
    let now = unix_now();
    let registry = RendezvousRegistry::with_public_only(true);
    assert!(registry.register(&reg, now).is_err());
}

#[test]
fn default_registry_stays_permissive_for_same_machine() {
    // 宽松默认：同机部署/单测依赖全 loopback 注册的可发现性，不得被 E5 收紧破坏
    let reg = loopback_reg("room-a");
    let registry = RendezvousRegistry::new();
    registry
        .register(&reg, unix_now())
        .expect("默认宽松策略保留同机可发现性");
}

#[test]
fn public_only_accepts_routable_and_mixed() {
    let now = unix_now();
    let registry = RendezvousRegistry::with_public_only(true);
    // 私网可路由（同 NAT 直连用途），整单接受
    let (kp, reg) = valid_reg("room-a");
    let _ = kp;
    registry.register(&reg, now).expect("私网可路由注册应接受");
    // 混合地址整单接受（签名记录不可改写，不做部分剥离）
    let mixed_reg = {
        let kp = Keypair::generate();
        let addrs = vec![
            TransportAddr::Quic {
                ip: "127.0.0.1".parse().unwrap(),
                port: 40001,
            },
            TransportAddr::Quic {
                ip: "203.0.113.7".parse().unwrap(),
                port: 40002,
            },
        ];
        sign_register(&kp, "room-a", &addrs, 60, unix_now())
    };
    registry
        .register(&mixed_reg, now)
        .expect("混合地址注册应接受");
}
