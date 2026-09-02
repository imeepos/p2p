//! 传输层契约：QUIC 优先（quinn），TCP 兜底（tokio + yamux）。
//!
//! dial 产出已完成安全握手、已互认身份的 [SecureConn]（实现归内核会话 K）。

use std::net::IpAddr;
use std::sync::Arc;

use p2p_identity::{Keypair, PeerId};
use p2p_mux::MuxControl;

/// 可拨号的传输地址。展示格式：`ip/u端口`（QUIC）、`ip/t端口`（TCP）。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TransportAddr {
    Quic { ip: IpAddr, port: u16 },
    Tcp { ip: IpAddr, port: u16 },
}

impl std::fmt::Display for TransportAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Quic { ip, port } => write!(f, "{ip}/u{port}"),
            Self::Tcp { ip, port } => write!(f, "{ip}/t{port}"),
        }
    }
}

/// 已完成安全握手与身份互认的连接。
pub struct SecureConn {
    /// 对端身份：握手时从证书/Noise 静态密钥推导，不可由对端自报字符串冒充。
    pub remote: PeerId,
    pub mux: Arc<dyn MuxControl>,
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("dial {addr}: {reason}")]
    Dial { addr: String, reason: String },
    #[error("handshake: {0}")]
    Handshake(String),
    #[error("peer mismatch: expected {expected}, got {actual}")]
    PeerMismatch { expected: String, actual: String },
}

#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// 拨号并完成安全升级。expected 为 Some 时对端身份必须一致。
    async fn dial(
        &self,
        addr: &TransportAddr,
        keypair: &Keypair,
        expected: Option<PeerId>,
    ) -> Result<SecureConn, TransportError>;
}
mod quic;
mod tcp;

pub use quic::QuicTransport;
pub use tcp::TcpTransport;

pub use p2p_mux::MAX_STREAMS_PER_CONN;
