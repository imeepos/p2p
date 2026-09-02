//! QUIC 的 TLS1.3 配置构造：双向证书均为身份扩展自签证书。

use std::sync::Arc;

use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};

use p2p_identity::Keypair;

use crate::tls_cert::build_identity_cert;
use crate::tls_verify::IdentityVerifier;
use crate::SecurityError;

/// QUIC ALPN 标识，握手层一致性由 ALPN 协商保证
pub const QUIC_ALPN: &[u8] = b"p2p-base/1";

pub fn quic_client_config(keypair: &Keypair) -> Result<rustls::ClientConfig, SecurityError> {
    let (cert, pkcs8) = build_identity_cert(keypair)?;
    let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(pkcs8));
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(tls_err)?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(IdentityVerifier))
        .with_client_auth_cert(vec![cert], key_der)
        .map_err(tls_err)?;
    Ok(with_alpn(config))
}

pub fn quic_server_config(keypair: &Keypair) -> Result<rustls::ServerConfig, SecurityError> {
    let (cert, pkcs8) = build_identity_cert(keypair)?;
    let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(pkcs8));
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(tls_err)?
        .with_client_cert_verifier(Arc::new(IdentityVerifier))
        .with_single_cert(vec![cert], key_der)
        .map_err(tls_err)?;
    Ok(with_alpn(config))
}

fn tls_err(e: rustls::Error) -> SecurityError {
    SecurityError::Handshake(format!("rustls: {e}"))
}

fn with_alpn<C: HasAlpn>(mut config: C) -> C {
    config.set_alpn(vec![QUIC_ALPN.to_vec()]);
    config
}

trait HasAlpn {
    fn set_alpn(&mut self, alpn: Vec<Vec<u8>>);
}

impl HasAlpn for rustls::ClientConfig {
    fn set_alpn(&mut self, alpn: Vec<Vec<u8>>) {
        self.alpn_protocols = alpn;
    }
}

impl HasAlpn for rustls::ServerConfig {
    fn set_alpn(&mut self, alpn: Vec<Vec<u8>>) {
        self.alpn_protocols = alpn;
    }
}