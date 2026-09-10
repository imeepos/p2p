//! rendezvous-register.json 消费：SignedFields、签名与防篡改/重放语义。

mod common;
use common::{case_name, cases, load, unhex};
use p2p_discovery::rendezvous::messages as rv;
use p2p_identity::{Keypair, PeerId};
use p2p_transport::TransportAddr;
use prost::Message;
use std::net::IpAddr;

const FILE: &str = "rendezvous-register.json";

fn addr_of(entry: &serde_json::Value) -> TransportAddr {
    let ip: IpAddr = entry["ip"].as_str().unwrap().parse().unwrap();
    let port = entry["port"].as_u64().unwrap() as u16;
    if entry["quic"].as_bool().unwrap() {
        TransportAddr::Quic { ip, port }
    } else {
        TransportAddr::Tcp { ip, port }
    }
}

fn golden() -> (Keypair, rv::Register, Vec<u8>, u64) {
    let doc = load(FILE);
    let case = cases(&doc)
        .into_iter()
        .find(|c| case_name(c) == "register_golden")
        .expect("register_golden case");
    let seed: [u8; 32] = unhex(case["seed_hex"].as_str().unwrap())
        .try_into()
        .unwrap();
    let kp = Keypair::from_seed(&seed);
    let addrs: Vec<TransportAddr> = case["addrs"]
        .as_array()
        .unwrap()
        .iter()
        .map(addr_of)
        .collect();
    let namespace = case["namespace"].as_str().unwrap();
    let ttl: u32 = case["ttl_secs"].as_u64().unwrap() as u32;
    let issued_at: u64 = case["issued_at"].as_str().unwrap().parse().unwrap();

    let signed = rv::signed_payload(namespace, &kp.peer_id(), &addrs, ttl, issued_at);
    assert_eq!(
        common::hex(&signed),
        case["signed_fields_hex"].as_str().unwrap(),
        "被签字节不符"
    );
    let reg = rv::sign_register(&kp, namespace, &addrs, ttl, issued_at);
    assert_eq!(
        common::hex(&reg.encode_to_vec()),
        case["register_protobuf_hex"].as_str().unwrap(),
        "Register 编码不符"
    );
    let wire = unhex(case["register_protobuf_hex"].as_str().unwrap());
    (kp, reg, wire, issued_at)
}

#[test]
fn register_golden_matches_implementation() {
    let (kp, reg, wire, issued_at) = golden();
    assert_eq!(
        reg.peer_id,
        kp.peer_id().as_bytes().to_vec(),
        "peer_id 必须与种子身份绑定"
    );
    assert_eq!(
        reg.sig,
        kp.sign(&rv::signed_payload(
            &reg.namespace,
            &PeerId::from_bytes(reg.peer_id.clone().try_into().unwrap()),
            &reg.addrs
                .iter()
                .map(|a| a.to_addr().unwrap())
                .collect::<Vec<_>>(),
            reg.ttl_secs,
            reg.issued_at,
        ))
    );
    let decoded = rv::Register::decode(wire.as_slice()).expect("向量 protobuf 须可解码");
    assert_eq!(decoded, reg, "解码结果与实现构造不一致");
    assert!(rv::verify_register(&reg, issued_at), "黄金注册必须验签通过");
}

#[test]
fn ttl_tamper_breaks_signature() {
    let (_kp, mut reg, _wire, issued_at) = golden();
    let doc = load(FILE);
    let case = cases(&doc)
        .into_iter()
        .find(|c| case_name(c) == "ttl_tamper_rejected")
        .expect("ttl_tamper_rejected case");
    reg.ttl_secs = case["operation"]["set"]["ttl_secs"].as_u64().unwrap() as u32;
    assert!(
        !rv::verify_register(&reg, issued_at),
        "改 TTL 后验签必须失败"
    );
}

#[test]
fn freshness_window_rejects_stale_registration() {
    let (_kp, reg, _wire, _issued_at) = golden();
    let doc = load(FILE);
    let case = cases(&doc)
        .into_iter()
        .find(|c| case_name(c) == "freshness_window_rejected")
        .expect("freshness_window_rejected case");
    let verify_now: u64 = case["verify_now"].as_str().unwrap().parse().unwrap();
    assert!(!rv::verify_register(&reg, verify_now), "超容差必须拒");
}

/// 消融测试：把向量签名在内存翻转一字节，验签必须变红。
#[test]
fn signature_bit_flip_breaks_verification() {
    let (kp, mut reg, _wire, issued_at) = golden();
    reg.sig[0] ^= 0x01;
    assert!(
        !rv::verify_register(&reg, issued_at),
        "翻转签名一字节后必须失败"
    );
    let mut signed = rv::signed_payload(
        &reg.namespace,
        &kp.peer_id(),
        &reg.addrs
            .iter()
            .map(|a| a.to_addr().unwrap())
            .collect::<Vec<_>>(),
        reg.ttl_secs,
        reg.issued_at,
    );
    signed[0] ^= 0x01;
    let sig: [u8; 64] = reg.sig.clone().try_into().unwrap();
    assert!(
        !Keypair::verify(&kp.public(), &signed, &sig),
        "翻转被签字节后必须失败"
    );
}
