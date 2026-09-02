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
    let reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
    assert!(verify_register(&reg));
}

#[test]
fn tampered_addrs_rejected() {
    let kp = Keypair::generate();
    let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
    reg.addrs[0].port = 9999;
    assert!(!verify_register(&reg));
}

#[test]
fn tampered_namespace_rejected() {
    let kp = Keypair::generate();
    let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
    reg.namespace = "room-b".to_string();
    assert!(!verify_register(&reg));
}

#[test]
fn wrong_peer_id_rejected() {
    let kp = Keypair::generate();
    let other = Keypair::generate();
    let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
    reg.peer_id = other.peer_id().as_bytes().to_vec();
    assert!(!verify_register(&reg));
}

#[test]
fn wrong_signature_rejected() {
    let kp = Keypair::generate();
    let attacker = Keypair::generate();
    let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
    reg.sig = attacker
        .sign(&signed_payload("room-a", &kp.peer_id(), &sample_addrs()))
        .to_vec();
    assert!(!verify_register(&reg));
}

#[test]
fn prost_roundtrip_register_query_response() {
    let kp = Keypair::generate();
    let reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
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
    let reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
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
