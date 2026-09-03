//! 传输层契约：QUIC 优先（quinn），TCP 兜底（tokio + yamux）。
//!
//! dial 产出已完成安全握手、已互认身份的 [SecureConn]（实现归内核会话 K）。

use std::io;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

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

impl TransportAddr {
    /// 跨网可拨性判定（rendezvous 地址卫生，E5）：loopback 只在本机有效；
    /// v4 link-local（169.254/16）与 v6 链路本地（fe80::/10）缺接口作用域，
    /// 注册/发现语义下均不可拨。私网地址保留（同 NAT 直连合法用途）。
    pub fn is_routable(&self) -> bool {
        let (Self::Quic { ip, .. } | Self::Tcp { ip, .. }) = self;
        match ip {
            IpAddr::V4(v4) => !v4.is_loopback() && !v4.is_link_local(),
            IpAddr::V6(v6) => {
                let o = v6.octets();
                let link_local = o[0] == 0xfe && (o[1] & 0xc0) == 0x80;
                !v6.is_loopback() && !link_local
            }
        }
    }
}

/// 已完成安全握手与身份互认的连接。
pub struct SecureConn {
    /// 对端身份：握手时从证书/Noise 静态密钥推导，不可由对端自报字符串冒充。
    pub remote: PeerId,
    pub mux: Arc<dyn MuxControl>,
}

/// source 链载体：装箱内层错误（E7-K2 错误链保真），调用方沿
/// `std::error::Error::source` 逐层 downcast 还原内层类型与文案。
pub type ChainedError = Box<dyn std::error::Error + Send + Sync>;

/// E7-K2 错误链载体：std 的 `io::Error::source()` 只返回载荷自身的 source
/// 而不返回载荷，直接 `io::Error::new(kind, inner)` 会令 source() 遍历盲视内层。
/// 以本包装器作载荷：`err.to_string()` 保持内层原文，`err.source()` 即内层错误。
#[derive(Debug)]
pub(crate) struct ChainedPayload<E> {
    inner: E,
}

impl<E: std::fmt::Display> std::fmt::Display for ChainedPayload<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ChainedPayload<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.inner)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// 无内层错误对象可保留的场景：地址族不匹配属契约性拒绝，纯文本即完整语义。
    #[error("dial {addr}: {reason}")]
    Dial { addr: String, reason: String },
    /// 无内层错误对象可保留的场景：证书结构缺陷（Option/downcast 失败），只有文案。
    #[error("handshake: {0}")]
    Handshake(String),
    #[error("peer mismatch: expected {expected}, got {actual}")]
    PeerMismatch { expected: String, actual: String },
    /// E7-K2 新增：dial 失败且内层错误（io / quinn ConnectError / 超时）挂在 source 链。
    #[error("dial {addr}: {source}")]
    DialChained {
        addr: String,
        #[source]
        source: ChainedError,
    },
    /// E7-K2 新增：握手失败且内层错误（SecurityError / 证书解析）挂在 source 链。
    #[error("handshake: {source}")]
    HandshakeChained {
        #[source]
        source: ChainedError,
    },
}

/// 超时拨号错误：io 层 TimedOut 载文案，文案即 what+时限，无更深内层对象。
pub(crate) fn dial_timeout(addr: &TransportAddr, timeout: Duration, what: &str) -> TransportError {
    TransportError::DialChained {
        addr: addr.to_string(),
        source: Box::new(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("{what} timeout after {timeout:?}"),
        )),
    }
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

#[cfg(test)]
mod addr_tests {
    use super::*;

    fn quic(ip: &str) -> TransportAddr {
        TransportAddr::Quic {
            ip: ip.parse().unwrap(),
            port: 4000,
        }
    }

    #[test]
    fn loopback_and_link_local_not_routable() {
        assert!(!quic("127.0.0.1").is_routable());
        assert!(!quic("::1").is_routable());
        assert!(!quic("169.254.3.4").is_routable());
        assert!(!quic("fe80::1").is_routable());
    }

    #[test]
    fn public_private_and_global_v6_routable() {
        assert!(quic("203.0.113.7").is_routable());
        assert!(quic("10.0.0.5").is_routable());
        assert!(quic("192.168.1.5").is_routable());
        assert!(quic("240e:abcd::1").is_routable());
    }

    /// E7-K2 白盒回归：超时拨号错误必须携带 TimedOut io source。
    /// 消融：换回纯文本 Dial 后本用例转红（DialChained 不再产出）。
    #[test]
    fn dial_timeout_carries_timed_out_io_source() {
        let err = dial_timeout(&quic("203.0.113.7"), Duration::from_secs(3), "connect");
        match &err {
            TransportError::DialChained { source, .. } => {
                let io_src = source
                    .downcast_ref::<std::io::Error>()
                    .expect("timeout source must be io::Error");
                assert_eq!(io_src.kind(), std::io::ErrorKind::TimedOut);
                assert!(
                    io_src.to_string().contains("connect timeout after"),
                    "timeout context must survive, got: {io_src}"
                );
            }
            other => panic!("expected DialChained, got {other:?}"),
        }
    }
}
