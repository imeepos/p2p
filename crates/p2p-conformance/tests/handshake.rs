//! handshake-identity.json 消费：96 字节身份负载、域串验签与 TLS 常量。

mod common;
use common::{case_name, cases, load, unhex};
use der::Decode as _;
use ed25519_dalek::VerifyingKey;
use p2p_identity::Keypair;
use p2p_security::build_identity_cert;
use x509_cert::Certificate;

const FILE: &str = "handshake-identity.json";

fn golden_case() -> serde_json::Value {
    cases(&load(FILE))
        .into_iter()
        .find(|c| case_name(c) == "identity_payload_golden")
        .expect("identity_payload_golden case")
        .clone()
}

fn x25519_of(pubkey: &[u8; 32]) -> [u8; 32] {
    *VerifyingKey::from_bytes(pubkey)
        .expect("ed25519 公钥解码")
        .to_montgomery()
        .as_bytes()
}

fn signed_message(domain: &[u8], x25519: &[u8; 32]) -> Vec<u8> {
    let mut msg = domain.to_vec();
    msg.extend_from_slice(x25519);
    msg
}

#[test]
fn identity_payload_chain_matches_vector() {
    let case = golden_case();
    let seed: [u8; 32] = unhex(case["seed_hex"].as_str().unwrap())
        .try_into()
        .unwrap();
    let kp = Keypair::from_seed(&seed);
    let pubkey = kp.public();

    assert_eq!(
        common::hex(&pubkey),
        case["public_key_hex"].as_str().unwrap(),
        "ed25519 公钥不符"
    );
    assert_eq!(
        common::hex(&x25519_of(&pubkey)),
        case["x25519_static_hex"].as_str().unwrap(),
        "X25519 静态公钥（蒙哥马利映射）不符"
    );

    let domain = unhex(case["sign_domain_hex"].as_str().unwrap());
    let signed = signed_message(&domain, &x25519_of(&pubkey));
    assert_eq!(
        common::hex(&signed),
        case["signed_message_hex"].as_str().unwrap(),
        "被签消息（域串 + X25519 静态钥）不符"
    );

    let sig = kp.sign(&signed);
    assert_eq!(
        common::hex(&sig),
        case["signature_hex"].as_str().unwrap(),
        "签名不符（ed25519 确定性）"
    );

    let payload = unhex(case["identity_payload_hex"].as_str().unwrap());
    assert_eq!(
        payload.len(),
        case["identity_payload_len"].as_u64().unwrap() as usize,
        "负载长度须为 96"
    );
    assert_eq!(
        &payload[..32],
        &pubkey[..],
        "负载 [0..32) 必须是 ed25519 公钥"
    );
    assert_eq!(&payload[32..], &sig[..], "负载 [32..96) 必须是签名");
    assert!(
        Keypair::verify(&pubkey, &signed, &sig),
        "黄金负载必须验签通过"
    );
}

/// 消融测试：向量负载在内存翻转一字节，验签必须变红。
#[test]
fn tampered_payload_fails_verification() {
    let case = cases(&load(FILE))
        .into_iter()
        .find(|c| case_name(c) == "signature_tamper_rejected")
        .expect("signature_tamper_rejected case")
        .clone();
    let payload = unhex(case["identity_payload_hex"].as_str().unwrap());
    let domain = unhex(case["sign_domain_hex"].as_str().unwrap());
    let mut pubkey = [0u8; 32];
    pubkey.copy_from_slice(&payload[..32]);
    let sig: [u8; 64] = payload[32..].try_into().unwrap();
    let signed = signed_message(&domain, &x25519_of(&pubkey));
    assert!(
        !Keypair::verify(&pubkey, &signed, &sig),
        "翻转签名字节后验签必须失败"
    );
}

#[test]
fn wrong_domain_fails_verification() {
    let case = cases(&load(FILE))
        .into_iter()
        .find(|c| case_name(c) == "wrong_domain_rejected")
        .expect("wrong_domain_rejected case")
        .clone();
    let pubkey: [u8; 32] = unhex(case["public_key_hex"].as_str().unwrap())
        .try_into()
        .unwrap();
    let signed = unhex(case["signed_message_hex"].as_str().unwrap());
    let sig: [u8; 64] = unhex(case["signature_hex"].as_str().unwrap())
        .try_into()
        .unwrap();
    // 被签消息域名不同（p2p-noise-xx-v2），同一签名必须验不过
    assert_ne!(&signed[..15], b"p2p-noise-xx-v1");
    assert!(
        !Keypair::verify(&pubkey, &signed, &sig),
        "异域串验签必须失败"
    );
}

#[test]
fn identity_cert_carries_vector_oid_and_pubkey() {
    let case = cases(&load(FILE))
        .into_iter()
        .find(|c| case_name(c) == "tls_identity_extension_oid")
        .expect("tls_identity_extension_oid case")
        .clone();
    let oid = case["oid"].as_str().unwrap();
    let seed: [u8; 32] = unhex(golden_case()["seed_hex"].as_str().unwrap())
        .try_into()
        .unwrap();
    let kp = Keypair::from_seed(&seed);
    let (cert, _pkcs8) = build_identity_cert(&kp).expect("构造身份证书");

    let parsed = Certificate::from_der(cert.as_ref()).expect("证书 DER 解析");
    let extensions = parsed
        .tbs_certificate
        .extensions
        .as_ref()
        .expect("身份证书必须带扩展");
    let hit = extensions.iter().find(|e| e.extn_id.to_string() == oid);
    let ext = hit.unwrap_or_else(|| panic!("证书必须携带身份扩展 OID {oid}"));
    assert_eq!(
        ext.extn_value.as_bytes(),
        &kp.public()[..],
        "扩展内容必须是 32 字节公钥"
    );
}

#[test]
fn quic_alpn_matches_vector() {
    let case = cases(&load(FILE))
        .into_iter()
        .find(|c| case_name(c) == "quic_alpn")
        .expect("quic_alpn case")
        .clone();
    assert_eq!(
        std::str::from_utf8(p2p_security::QUIC_ALPN).unwrap(),
        case["alpn"].as_str().unwrap(),
        "QUIC ALPN 与向量不符"
    );
}
