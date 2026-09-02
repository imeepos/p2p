//! QUIC 身份证书：ed25519 公钥嵌入自签证书扩展（libp2p-tls 思路）。
//!
//! 校验规则（见 tls_verify）：不查 CA，只认带身份扩展的自签证书；
//! TLS 握手的 CertificateVerify 已由 rustls 用证书公钥强制验签，
//! 持有证书私钥者才可能完成握手，PeerId 因此与密码学材料绑定。

use std::format;

use der::asn1::ObjectIdentifier;
use der::Decode;
use rustls::pki_types::CertificateDer;

use p2p_identity::{Keypair, PeerId};

use crate::SecurityError;

/// 身份扩展 OID（私有企业号空间，全底座唯一）
pub const IDENTITY_OID: &str = "1.3.6.1.4.1.59015.1";
/// ed25519 PKCS#8 前缀：DER SEQUENCE 头，后接 32 字节种子
const PKCS8_ED25519_PREFIX: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04,
    0x20,
];

fn identity_oid() -> ObjectIdentifier {
    ObjectIdentifier::new_unwrap(IDENTITY_OID)
}

fn identity_oid_parts() -> [u64; 8] {
    [1, 3, 6, 1, 4, 1, 59015, 1]
}

/// 种子转 PKCS#8 DER（ed25519 私钥封装），rcgen 与 rustls 共用
pub fn pkcs8_der(keypair: &Keypair) -> Vec<u8> {
    let mut der = Vec::with_capacity(48);
    der.extend_from_slice(&PKCS8_ED25519_PREFIX);
    der.extend_from_slice(&keypair.to_seed_bytes());
    der
}

/// 构造自签证书（含身份扩展），返回证书 DER 与 PKCS#8 私钥
pub fn build_identity_cert(keypair: &Keypair) -> Result<(CertificateDer<'static>, Vec<u8>), SecurityError> {
    let pkcs8 = pkcs8_der(keypair);
    let key_pair = rcgen::KeyPair::from_pkcs8_der_and_sign_algo(
        &rustls::pki_types::PrivatePkcs8KeyDer::from(pkcs8.clone()),
        &rcgen::PKCS_ED25519,
    )
    .map_err(|e| SecurityError::Handshake(format!("rcgen keypair: {e}")))?;

    let mut params = rcgen::CertificateParams::new(Vec::new())
        .map_err(|e| SecurityError::Handshake(format!("cert params: {e}")))?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "p2p-base-node");
    params.custom_extensions = vec![rcgen::CustomExtension::from_oid_content(
        &identity_oid_parts(),
        keypair.public().to_vec(),
    )];
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| SecurityError::Handshake(format!("self sign: {e}")))?;
    Ok((cert.der().clone(), pkcs8))
}

/// 从证书 DER 解析出身份扩展承载的 ed25519 公钥
pub fn identity_pub_from_cert(cert: &CertificateDer<'_>) -> Result<[u8; 32], SecurityError> {
    let der_bytes: &[u8] = cert.as_ref();
    let parsed = x509_cert::Certificate::from_der(der_bytes)
        .map_err(|e| SecurityError::Handshake(format!("cert der: {e}")))?;

    let extensions = parsed
        .tbs_certificate
        .extensions
        .as_ref()
        .ok_or(SecurityError::IdentityUnverified)?;
    let mut pubkey: Option<[u8; 32]> = None;
    for ext in extensions.iter() {
        if ext.extn_id == identity_oid() {
            let raw = ext.extn_value.as_bytes();
            if raw.len() != 32 {
                return Err(SecurityError::IdentityUnverified);
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(raw);
            pubkey = Some(key);
        }
    }
    let pubkey = pubkey.ok_or(SecurityError::IdentityUnverified)?;

    // 证书 SPKI 原始钥必须与扩展一致：扩展声明身份，SPKI 才是握手验签的钥
    let spki = &parsed.tbs_certificate.subject_public_key_info;
    let Some(spki_raw) = spki.subject_public_key.as_bytes() else {
        return Err(SecurityError::IdentityUnverified);
    };
    if spki_raw != pubkey {
        return Err(SecurityError::IdentityUnverified);
    }
    Ok(pubkey)
}

/// 证书 -> PeerId（sha256(ed25519 公钥)，与 identity 层一致）
pub fn peer_id_from_cert(cert: &CertificateDer<'_>) -> Result<PeerId, SecurityError> {
    let pubkey = identity_pub_from_cert(cert)?;
    Ok(PeerId::from_public_key(&pubkey))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cert_carries_identity_and_peer_id_roundtrip() {
        let keypair = Keypair::generate();
        let (cert, pkcs8) = build_identity_cert(&keypair).expect("build cert");
        assert_eq!(pkcs8.len(), 48);

        let peer = peer_id_from_cert(&cert).expect("extract peer");
        assert_eq!(peer, keypair.peer_id());
    }

    #[test]
    fn cert_without_identity_extension_is_rejected() {
        // 用 rcgen 生成一张不带身份扩展的普通证书
        let key_pair = rcgen::KeyPair::generate().expect("rcgen keygen");
        let params = rcgen::CertificateParams::new(Vec::new()).expect("params");
        let cert = params.self_signed(&key_pair).expect("self sign");
        assert!(peer_id_from_cert(cert.der()).is_err());
    }
}