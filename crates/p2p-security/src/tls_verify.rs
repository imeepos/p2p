//! 自定义证书校验器：只认内嵌身份扩展的自签证书，握手签名用扩展公钥验证。

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DigitallySignedStruct, DistinguishedName, Error, SignatureScheme};

use crate::tls_cert::identity_pub_from_cert;

fn error_cert(reason: &str) -> Error {
    Error::General(format!("p2p identity cert: {reason}"))
}

/// 统一校验逻辑：证书内含身份扩展，且握手签名可被扩展公钥验证。
#[derive(Debug)]
pub struct IdentityVerifier;

impl IdentityVerifier {
    fn verify_handshake_sig(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        if dss.scheme != SignatureScheme::ED25519 {
            return Err(error_cert("only ed25519 handshake signatures accepted"));
        }
        let pubkey = identity_pub_from_cert(cert).map_err(|e| error_cert(&e.to_string()))?;
        let key = ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, pubkey);
        key.verify(message, dss.signature())
            .map(|_| HandshakeSignatureValid::assertion())
            .map_err(|_| error_cert("handshake signature invalid"))
    }
}

impl ServerCertVerifier for IdentityVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        identity_pub_from_cert(end_entity)
            .map(|_| ServerCertVerified::assertion())
            .map_err(|e| error_cert(&e.to_string()))
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.verify_handshake_sig(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.verify_handshake_sig(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }
}

impl ClientCertVerifier for IdentityVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, Error> {
        identity_pub_from_cert(end_entity)
            .map(|_| ClientCertVerified::assertion())
            .map_err(|e| error_cert(&e.to_string()))
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.verify_handshake_sig(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.verify_handshake_sig(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }
}
