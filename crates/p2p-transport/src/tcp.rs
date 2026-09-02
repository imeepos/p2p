//! TCP 传输（tokio）：Noise XX 升级 + yamux 复用。

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};

use p2p_identity::{Keypair, PeerId};
use p2p_mux::{BoxedStream, YamuxMux, MAX_STREAMS_PER_CONN};
use p2p_security::{NoiseXx, SecurityError, SecurityUpgrade};

use crate::{SecureConn, Transport, TransportAddr, TransportError};

fn security_err(e: SecurityError) -> TransportError {
    match e {
        SecurityError::PeerMismatch { expected, actual } => TransportError::PeerMismatch {
            expected,
            actual,
        },
        other => TransportError::Handshake(other.to_string()),
    }
}

fn dial_err(addr: &TransportAddr, e: impl std::fmt::Display) -> TransportError {
    TransportError::Dial { addr: addr.to_string(), reason: e.to_string() }
}

/// TCP 传输：无状态，可多份复用。
pub struct TcpTransport {
    noise: NoiseXx,
}

impl TcpTransport {
    pub fn new() -> Self {
        Self { noise: NoiseXx }
    }

    /// 监听 TCP 入站。
    pub async fn bind(&self, addr: SocketAddr) -> io::Result<TcpListener> {
        TcpListener::bind(addr).await
    }

    /// 接受一条入站连接：Noise 升级 -> yamux -> SecureConn。
    /// 单次失败原样返回 Err，调用方决定是否继续循环（失败留日志由内部完成）。
    pub async fn accept(
        &self,
        listener: &TcpListener,
        keypair: &Keypair,
    ) -> io::Result<SecureConn> {
        let (stream, peer_addr) = listener.accept().await?;
        if let Err(e) = stream.set_nodelay(true) {
            tracing::warn!(%peer_addr, error = %e, "tcp set_nodelay failed");
        }
        let boxed: BoxedStream = Box::new(stream);
        let (remote, upgraded) = self
            .noise
            .inbound(boxed, keypair)
            .await
            .map_err(|e| {
                tracing::warn!(%peer_addr, error = %e, "tcp inbound handshake failed");
                io::Error::new(io::ErrorKind::InvalidData, e.to_string())
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
                return Err(dial_err(addr, "tcp transport cannot dial a quic address"));
            }
        };
        let stream = TcpStream::connect(SocketAddr::new(ip, port))
            .await
            .map_err(|e| dial_err(addr, e))?;
        stream.set_nodelay(true).map_err(|e| dial_err(addr, e))?;
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
