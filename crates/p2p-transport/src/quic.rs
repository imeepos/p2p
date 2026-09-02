//! QUIC 传输（quinn）：TLS1.3 证书内嵌身份，见 p2p-security::tls。

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::rustls::pki_types::CertificateDer;

use p2p_identity::{Keypair, PeerId};
use p2p_mux::{QuicMux, MAX_STREAMS_PER_CONN};
use p2p_security::{peer_id_from_cert, quic_client_config, quic_server_config};

use crate::{SecureConn, Transport, TransportAddr, TransportError};

/// 身份校验不依赖证书域名，SNI 用固定占位即可
const SERVER_NAME: &str = "p2p-base";
const KEEP_ALIVE: Duration = Duration::from_secs(10);
/// QUIC 空闲连接回收上限：半开连接不得长期占用（安全审查 1 期 M3）
pub const QUIC_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// QUIC 握手（connecting/Incoming resolve）总时长上限
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

fn stream_limits() -> Arc<quinn::TransportConfig> {
    let mut transport = quinn::TransportConfig::default();
    // 协议级并发流上限，与复用层信号量双重防护
    transport.max_concurrent_bidi_streams(
        quinn::VarInt::from_u64(MAX_STREAMS_PER_CONN as u64).expect("stream limit fits varint"),
    );
    transport.max_idle_timeout(Some(
        quinn::IdleTimeout::try_from(QUIC_IDLE_TIMEOUT).expect("idle timeout fits varint"),
    ));
    Arc::new(transport)
}

fn peer_id_of(conn: &quinn::Connection) -> Result<PeerId, TransportError> {
    let identity = conn
        .peer_identity()
        .ok_or_else(|| TransportError::Handshake("no peer certificate".into()))?;
    let certs = identity
        .downcast::<Vec<CertificateDer<'static>>>()
        .map_err(|_| TransportError::Handshake("peer identity is not rustls certs".into()))?;
    let end_entity = certs
        .first()
        .ok_or_else(|| TransportError::Handshake("empty peer certificate chain".into()))?;
    peer_id_from_cert(end_entity).map_err(|e| TransportError::Handshake(e.to_string()))
}

fn secure_conn(
    conn: quinn::Connection,
    expected: Option<PeerId>,
) -> Result<SecureConn, TransportError> {
    let remote = peer_id_of(&conn)?;
    if let Some(expected) = expected {
        if expected != remote {
            return Err(TransportError::PeerMismatch {
                expected: expected.to_string(),
                actual: remote.to_string(),
            });
        }
    }
    let mux = Arc::new(QuicMux::new(conn, MAX_STREAMS_PER_CONN));
    Ok(SecureConn { remote, mux })
}

/// QUIC 传输：单端点同时支持拨号与监听。
pub struct QuicTransport {
    endpoint: quinn::Endpoint,
}

impl QuicTransport {
    /// 仅拨号端：绑定随机本地端口。
    pub fn new() -> io::Result<Self> {
        let endpoint = quinn::Endpoint::client(SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 0))?;
        Ok(Self { endpoint })
    }

    /// 监听端：以给定身份接受入站 QUIC。
    pub async fn bind(addr: SocketAddr, keypair: &Keypair) -> io::Result<Self> {
        let crypto = quic_server_config(keypair)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("server tls: {e}")))?;
        let quic_crypto = QuicServerConfig::try_from(Arc::new(crypto))
            .map_err(|e| io::Error::other(format!("quic crypto: {e}")))?;
        let mut server = quinn::ServerConfig::with_crypto(Arc::new(quic_crypto));
        server.transport_config(stream_limits());
        let endpoint = quinn::Endpoint::server(server, addr)?;
        Ok(Self { endpoint })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.endpoint.local_addr()
    }

    /// 关停：向全部存量连接发送应用层关闭（APPLICATION_CLOSE），端点随后
    /// 不再接受新连接；挂起的 accept()/dial() 以既有错误路径结束，不悬挂。
    pub fn close(&self) {
        self.endpoint
            .close(quinn::VarInt::from_u32(0), b"transport shutdown");
    }

    /// 接受下一条入站连接并升级为 SecureConn；endpoint 关闭后返回 None。
    /// 入站 QUIC 握手限时，升级失败记日志并返回 None，不中断监听循环。
    pub async fn accept(&self) -> Option<SecureConn> {
        let incoming = self.endpoint.accept().await?;
        match tokio::time::timeout(HANDSHAKE_TIMEOUT, incoming).await {
            Ok(Ok(conn)) => match secure_conn(conn, None) {
                Ok(conn) => Some(conn),
                Err(e) => {
                    tracing::warn!(error = %e, "quic inbound identity upgrade failed");
                    None
                }
            },
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "quic inbound handshake failed");
                None
            }
            Err(_) => {
                tracing::warn!(
                    timeout = ?HANDSHAKE_TIMEOUT,
                    "quic inbound handshake timed out"
                );
                None
            }
        }
    }
}

#[async_trait::async_trait]
impl Transport for QuicTransport {
    async fn dial(
        &self,
        addr: &TransportAddr,
        keypair: &Keypair,
        expected: Option<PeerId>,
    ) -> Result<SecureConn, TransportError> {
        let (ip, port) = match addr {
            TransportAddr::Quic { ip, port } => (*ip, *port),
            TransportAddr::Tcp { .. } => {
                return Err(TransportError::Dial {
                    addr: addr.to_string(),
                    reason: "quic transport cannot dial a tcp address".into(),
                });
            }
        };
        let crypto =
            quic_client_config(keypair).map_err(|e| TransportError::Handshake(e.to_string()))?;
        let quic_crypto = QuicClientConfig::try_from(Arc::new(crypto))
            .map_err(|e| TransportError::Handshake(format!("quic crypto: {e}")))?;
        let mut client = quinn::ClientConfig::new(Arc::new(quic_crypto));
        let mut transport = quinn::TransportConfig::default();
        transport.keep_alive_interval(Some(KEEP_ALIVE));
        transport.max_idle_timeout(Some(
            quinn::IdleTimeout::try_from(QUIC_IDLE_TIMEOUT).expect("idle timeout fits varint"),
        ));
        client.transport_config(Arc::new(transport));

        let connecting = self
            .endpoint
            .connect_with(client, SocketAddr::new(ip, port), SERVER_NAME)
            .map_err(|e| TransportError::Dial {
                addr: addr.to_string(),
                reason: e.to_string(),
            })?;
        let conn = match tokio::time::timeout(HANDSHAKE_TIMEOUT, connecting).await {
            Ok(Ok(conn)) => conn,
            Ok(Err(e)) => {
                return Err(TransportError::Dial {
                    addr: addr.to_string(),
                    reason: e.to_string(),
                });
            }
            Err(_) => {
                return Err(TransportError::Dial {
                    addr: addr.to_string(),
                    reason: format!("quic handshake timeout after {HANDSHAKE_TIMEOUT:?}"),
                });
            }
        };
        secure_conn(conn, expected)
    }
}
