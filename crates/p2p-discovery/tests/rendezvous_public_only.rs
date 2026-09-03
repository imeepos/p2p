//! rendezvous 服务端地址卫生（E5，2026-09-03 邻居表 127.0.0.1 复盘）：
//! 公共策略拒收全 loopback/link-local 注册；默认宽松保留同机可发现性。
//! 签名记录不可改写，只整单拒绝不部分剥离——混合地址必须整单接受。

use p2p_discovery::rendezvous::messages::{sign_register, unix_now, Query, Register};
use p2p_discovery::rendezvous::server::RendezvousRegistry;
use p2p_identity::Keypair;
use p2p_transport::TransportAddr;

fn reg_with(namespace: &str, ips: &[&str]) -> Register {
    let kp = Keypair::generate();
    let addrs: Vec<TransportAddr> = ips
        .iter()
        .map(|ip| TransportAddr::Quic {
            ip: ip.parse().unwrap(),
            port: 40000,
        })
        .collect();
    sign_register(&kp, namespace, &addrs, 60, unix_now())
}

fn stored_count(registry: &RendezvousRegistry, namespace: &str, peer: &[u8]) -> usize {
    registry
        .query(&Query {
            namespace: namespace.into(),
            peer_id: peer.to_vec(),
        })
        .peers
        .len()
}

#[test]
fn public_only_rejects_all_loopback_and_stores_nothing() {
    let reg = reg_with("room-a", &["127.0.0.1"]);
    let now = unix_now();
    let strict = RendezvousRegistry::with_public_only(true);
    assert!(strict.register(&reg, now).is_err());
    assert_eq!(
        stored_count(&strict, "room-a", &reg.peer_id),
        0,
        "拒收不得入库"
    );
}

#[test]
fn permissive_default_keeps_loopback_for_same_machine() {
    let reg = reg_with("room-a", &["127.0.0.1"]);
    let lax = RendezvousRegistry::new();
    lax.register(&reg, unix_now())
        .expect("默认宽松保留同机可发现性");
    assert_eq!(stored_count(&lax, "room-a", &reg.peer_id), 1);
}

#[test]
fn public_only_accepts_routable_and_mixed() {
    let now = unix_now();
    let strict = RendezvousRegistry::with_public_only(true);
    // 私网可路由（同 NAT 直连合法用途），接受
    let lan = reg_with("room-a", &["192.168.1.5"]);
    strict.register(&lan, now).expect("私网可路由注册应接受");
    // 混合地址整单接受：签名覆盖 addrs，服务端不做部分剥离
    let mixed = reg_with("room-a", &["127.0.0.1", "203.0.113.7"]);
    strict.register(&mixed, now).expect("混合地址注册应接受");
    assert_eq!(stored_count(&strict, "room-a", &mixed.peer_id), 1);
}
