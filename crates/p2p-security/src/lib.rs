//! 安全升级 seam（design §5.1/§6）：握手即认证，无明文阶段。
//!
//! QUIC：TLS1.3，证书内嵌身份公钥（类 libp2p-tls）；TCP：Noise XX（snow）。
//! 实现归内核会话 K。返回的对端 PeerId 必须从密码学材料推导，
//! 任何"对端自报身份"都不得直接采信。

use p2p_identity::{Keypair, PeerId};
use p2p_mux::BoxedStream;

#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("handshake aborted: {0}")]
    Handshake(String),
    #[error("remote identity unverifiable")]
    IdentityUnverified,
    #[error("peer mismatch: expected {expected}, got {actual}")]
    PeerMismatch { expected: String, actual: String },
}

#[async_trait::async_trait]
pub trait SecurityUpgrade: Send + Sync {
    /// 发起方：对原始字节流执行握手，返回对端身份与加密流。
    async fn outbound(
        &self,
        stream: BoxedStream,
        keypair: &Keypair,
        expected: Option<PeerId>,
    ) -> Result<(PeerId, BoxedStream), SecurityError>;

    /// 接收方：握手并返回对端身份与加密流。
    async fn inbound(
        &self,
        stream: BoxedStream,
        keypair: &Keypair,
    ) -> Result<(PeerId, BoxedStream), SecurityError>;
}
mod noise;
mod noise_stream;
mod tls;
mod tls_cert;
mod tls_verify;

pub use noise::NoiseXx;
pub use tls::{quic_client_config, quic_server_config, QUIC_ALPN};
pub use tls_cert::{build_identity_cert, peer_id_from_cert};
