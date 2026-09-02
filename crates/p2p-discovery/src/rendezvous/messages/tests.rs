use super::*;

fn sample_addrs() -> Vec<TransportAddr> {
    vec![
        TransportAddr::Quic {
            ip: "10.0.0.1".parse().unwrap(),
            port: 4000,
        },
        TransportAddr::Tcp {
            ip: "10.0.0.1".parse().unwrap(),
            port: 4001,
        },
    ]
}

#[test]
fn signed_register_passes_verification() {
    let kp = Keypair::generate();
    let now = unix_now();
    let reg = sign_register(&kp, "room-a", &sample_addrs(), 60, now);
    assert!(verify_register(&reg, now));
}

#[test]
fn tampered_addrs_rejected() {
    let kp = Keypair::generate();
    let now = unix_now();
    let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60, now);
    reg.addrs[0].port = 9999;
    assert!(!verify_register(&reg, now));
}

#[test]
fn tampered_namespace_rejected() {
    let kp = Keypair::generate();
    let now = unix_now();
    let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60, now);
    reg.namespace = "room-b".to_string();
    assert!(!verify_register(&reg, now));
}

#[test]
fn wrong_peer_id_rejected() {
    let kp = Keypair::generate();
    let other = Keypair::generate();
    let now = unix_now();
    let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60, now);
    reg.peer_id = other.peer_id().as_bytes().to_vec();
    assert!(!verify_register(&reg, now));
}

#[test]
fn wrong_signature_rejected() {
    let kp = Keypair::generate();
    let attacker = Keypair::generate();
    let now = unix_now();
    let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60, now);
    reg.sig = attacker
        .sign(&signed_payload(
            "room-a",
            &kp.peer_id(),
            &sample_addrs(),
            60,
            now,
        ))
        .to_vec();
    assert!(!verify_register(&reg, now));
}

#[test]
fn tampered_ttl_invalidates_signature() {
    // H1：TTL 已入签名，篡改后验签必败
    let kp = Keypair::generate();
    let now = unix_now();
    let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60, now);
    reg.ttl_secs = 999_999;
    assert!(!verify_register(&reg, now));
}

#[test]
fn tampered_issued_at_invalidates_signature() {
    // H1：注册时刻已入签名，篡改后验签必败
    let kp = Keypair::generate();
    let now = unix_now();
    let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60, now);
    reg.issued_at = now + 10_000;
    assert!(!verify_register(&reg, now));
}

#[test]
fn stale_and_future_timestamps_rejected() {
    // H1：重放窗口——旧签名在签发时刻有效，超出容差即拒
    let kp = Keypair::generate();
    let now = unix_now();
    let stale = sign_register(
        &kp,
        "room-a",
        &sample_addrs(),
        60,
        now - FRESH_TOLERANCE_SECS - 100,
    );
    assert!(verify_register(&stale, now - FRESH_TOLERANCE_SECS - 100));
    assert!(!verify_register(&stale, now));
    let future = sign_register(&kp, "room-a", &sample_addrs(), 60, now + 10_000);
    assert!(!verify_register(&future, now));
}

#[test]
fn unparseable_addr_rejects_whole_register() {
    // L2：任一地址畸形即整单拒绝，不静默丢弃
    let kp = Keypair::generate();
    let now = unix_now();
    let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60, now);
    reg.addrs[0].ip = "not-an-ip".to_string();
    assert!(!verify_register(&reg, now));
}

#[test]
fn port_overflow_rejected() {
    // L2：port 超 u16 范围显式拒绝，不做截断
    let mut msg = AddrMsg::from_addr(&sample_addrs()[0]);
    msg.port = 70_000;
    assert_eq!(msg.to_addr(), None);
}

#[test]
fn empty_addrs_allowed() {
    // 查询型节点可无监听地址：空地址列表签名有效、验签通过
    let kp = Keypair::generate();
    let now = unix_now();
    let reg = sign_register(&kp, "room-a", &[], 60, now);
    assert!(verify_register(&reg, now));
}

#[test]
fn prost_roundtrip_register_query_response() {
    let kp = Keypair::generate();
    let reg = sign_register(&kp, "room-a", &sample_addrs(), 60, unix_now());
    let decoded = Register::decode(reg.encode_to_vec().as_slice()).expect("decode");
    assert_eq!(reg, decoded);

    let query = Query {
        namespace: "room-a".into(),
        peer_id: reg.peer_id.clone(),
    };
    let q2 = Query::decode(query.encode_to_vec().as_slice()).expect("decode");
    assert_eq!(query, q2);

    let resp = Response {
        error: String::new(),
        peers: vec![PeerEntry {
            peer_id: reg.peer_id.clone(),
            addrs: reg.addrs.clone(),
        }],
    };
    let r2 = Response::decode(resp.encode_to_vec().as_slice()).expect("decode");
    assert_eq!(resp, r2);
}

#[test]
fn request_oneof_roundtrip() {
    let kp = Keypair::generate();
    let reg = sign_register(&kp, "room-a", &sample_addrs(), 60, unix_now());
    let req = Request::register(reg);
    let bytes = req.encode_to_vec();
    let decoded = Request::decode(bytes.as_slice()).expect("decode");
    assert_eq!(req, decoded);
    assert!(matches!(decoded.kind, Some(request::Kind::Register(_))));
}

#[test]
fn addr_conversion_roundtrip() {
    for addr in sample_addrs() {
        assert_eq!(AddrMsg::from_addr(&addr).to_addr(), Some(addr));
    }
}
