//! TCP 传输（tokio）：Noise XX 升级 + yamux 复用。

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use socket2::{SockRef, TcpKeepalive};
use tokio::net::{TcpListener, TcpStream};

use p2p_identity::{Keypair, PeerId};
use p2p_mux::{BoxedStream, YamuxMux, MAX_STREAMS_PER_CONN};
use p2p_security::{NoiseXx, SecurityError, SecurityUpgrade};

use crate::{SecureConn, Transport, TransportAddr, TransportError};

/// TCP 连接建立上限：不可达地址不得挂死拨号（安全审查 1 期 M3）
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// 空闲死链判定（E8-H3 与 QUIC 对齐）：SO_KEEPALIVE 起探时间取 QUIC 空闲
/// 回收上限、探测间隔对齐 QUIC keepalive，半开连接不再无限滞留。
/// best-effort：SockRef 仅借用 fd 不取得所有权，设置失败留 WARN，建连结果不变。
fn enable_keepalive(stream: &TcpStream, peer_addr: SocketAddr) {
    let ka = TcpKeepalive::new()
        .with_time(crate::quic::QUIC_IDLE_TIMEOUT)
        .with_interval(crate::quic::KEEP_ALIVE);
    if let Err(e) = SockRef::from(stream).set_tcp_keepalive(&ka) {
        tracing::warn!(%peer_addr, error = %e, "tcp set_keepalive failed");
    }
}

/// E7-K2 错误链保真：PeerMismatch 走结构化变体，其余 SecurityError 整体挂 source 链。
fn security_err(e: SecurityError) -> TransportError {
    match e {
        SecurityError::PeerMismatch { expected, actual } => {
            TransportError::PeerMismatch { expected, actual }
        }
        other => TransportError::HandshakeChained {
            source: Box::new(other),
        },
    }
}

/// E7-K2 错误链保真：io 内层错误整体挂 source 链，拒绝 to_string 拍平。
fn dial_err(
    addr: &TransportAddr,
    e: impl std::error::Error + Send + Sync + 'static,
) -> TransportError {
    TransportError::DialChained {
        addr: addr.to_string(),
        source: Box::new(e),
    }
}

/// TCP 传输：无状态，可多份复用。
pub struct TcpTransport {
    noise: NoiseXx,
    connect_timeout: Duration,
}

impl TcpTransport {
    pub fn new() -> Self {
        Self {
            noise: NoiseXx::new(),
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
        }
    }

    /// 覆盖超时：connect 与 Noise 握手分别独立计时（安全审查 1 期 M3）。
    pub fn with_timeouts(connect_timeout: Duration, handshake_timeout: Duration) -> Self {
        Self {
            noise: NoiseXx::new().with_handshake_timeout(handshake_timeout),
            connect_timeout,
        }
    }

    /// 监听 TCP 入站。
    pub async fn bind(&self, addr: SocketAddr) -> io::Result<TcpListener> {
        TcpListener::bind(addr).await
    }

    /// 接受一条入站连接：Noise 升级 -> yamux -> SecureConn。
    /// Noise 升级自带 deadline，半开握手到期即断；单次失败原样返回 Err，
    /// 调用方决定是否继续循环（失败留日志由内部完成）。
    pub async fn accept(
        &self,
        listener: &TcpListener,
        keypair: &Keypair,
    ) -> io::Result<SecureConn> {
        let (stream, peer_addr) = listener.accept().await?;
        if let Err(e) = stream.set_nodelay(true) {
            tracing::warn!(%peer_addr, error = %e, "tcp set_nodelay failed");
        }
        enable_keepalive(&stream, peer_addr);
        let boxed: BoxedStream = Box::new(stream);
        let (remote, upgraded) = self.noise.inbound(boxed, keypair).await.map_err(|e| {
            tracing::warn!(%peer_addr, error = %e, "tcp inbound handshake failed");
            // E7-K2：SecurityError 经 ChainedPayload 装箱，source() 遍历可达内层
            io::Error::new(
                io::ErrorKind::InvalidData,
                crate::ChainedPayload { inner: e },
            )
        })?;
        let mux = Arc::new(YamuxMux::new(upgraded, false, MAX_STREAMS_PER_CONN));
        Ok(SecureConn { remote, mux })
    }
}

impl Default for TcpTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Transport for TcpTransport {
    async fn dial(
        &self,
        addr: &TransportAddr,
        keypair: &Keypair,
        expected: Option<PeerId>,
    ) -> Result<SecureConn, TransportError> {
        let (ip, port) = match addr {
            TransportAddr::Tcp { ip, port } => (*ip, *port),
            TransportAddr::Quic { .. } => {
                // 契约性拒绝：无内层错误对象，纯文本 Dial 即完整语义
                return Err(TransportError::Dial {
                    addr: addr.to_string(),
                    reason: "tcp transport cannot dial a quic address".into(),
                });
            }
        };
        let peer = SocketAddr::new(ip, port);
        let connect = tokio::time::timeout(self.connect_timeout, TcpStream::connect(peer)).await;
        let stream = match connect {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => return Err(dial_err(addr, e)),
            Err(_) => {
                return Err(crate::dial_timeout(addr, self.connect_timeout, "connect"));
            }
        };
        stream.set_nodelay(true).map_err(|e| dial_err(addr, e))?;
        enable_keepalive(&stream, peer);
        let boxed: BoxedStream = Box::new(stream);
        let (remote, upgraded) = self
            .noise
            .outbound(boxed, keypair, expected)
            .await
            .map_err(security_err)?;
        let mux = Arc::new(YamuxMux::new(upgraded, true, MAX_STREAMS_PER_CONN));
        Ok(SecureConn { remote, mux })
    }
}
