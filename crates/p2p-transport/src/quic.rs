//! QUIC 传输（quinn）：TLS1.3 证书内嵌身份，见 p2p-security::tls。

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::rustls::pki_types::CertificateDer;
use socket2::{Domain, Protocol, Socket, Type};

use p2p_identity::{Keypair, PeerId};
use p2p_mux::{QuicMux, MAX_STREAMS_PER_CONN};
use p2p_security::{peer_id_from_cert, quic_client_config, quic_server_config};

use crate::{SecureConn, Transport, TransportAddr, TransportError};

/// 身份校验不依赖证书域名，SNI 用固定占位即可
const SERVER_NAME: &str = "p2p-base";
/// TCP 侧 keepalive 复用同一间隔（E8-H3 空闲维度统一，见 tcp.rs）。
pub(crate) const KEEP_ALIVE: Duration = Duration::from_secs(10);
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

/// 连接地址裁决（拨号出口统一过此关）：
/// - 未指定地址（0.0.0.0 / ::）契约性拒绝：映射成 v4-mapped 后 is_unspecified
///   失真，会绕过 quinn-proto 的确定性拒绝，退化为吃满握手超时的悬挂；
/// - 宿主无 IPv6（端点回退 IPv4-only）时 V6 目标契约性拒绝，报因可读；
/// - 双栈端点上 V4 目标以 v4-mapped（::ffff:a.b.c.d）表达，内核自动落 V4 路径。
fn connect_target(
    addr: &TransportAddr,
    ip: IpAddr,
    port: u16,
    dual_stack: bool,
) -> Result<SocketAddr, TransportError> {
    if ip.is_unspecified() {
        return Err(TransportError::Dial {
            addr: addr.to_string(),
            reason: "unspecified address is not dialable".into(),
        });
    }
    match (ip, dual_stack) {
        (IpAddr::V4(v4), true) => Ok(SocketAddr::new(IpAddr::V6(v4.to_ipv6_mapped()), port)),
        (IpAddr::V6(_), false) => Err(TransportError::Dial {
            addr: addr.to_string(),
            reason: "local quic endpoint is IPv4-only (host without IPv6); cannot dial IPv6".into(),
        }),
        _ => Ok(SocketAddr::new(ip, port)),
    }
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
    peer_id_from_cert(end_entity).map_err(|e| TransportError::HandshakeChained {
        source: Box::new(e),
    })
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
    /// 拨号端点是否双栈（[::]:0 且 V6ONLY 关）：决定 V4 目标是否走 v4-mapped。
    dual_stack: bool,
}

impl QuicTransport {
    /// 仅拨号端：优先绑 [::]:0 双栈（V6ONLY 关，V4 目标经 v4-mapped 出网，
    /// quinn 对 v4-mapped 目标原生支持，见 quinn-rs/quinn#1765）；宿主无 IPv6
    /// 时回退 0.0.0.0 并对 V6 目标显式拒拨。2026-09-04 线上事故：单绑 0.0.0.0
    /// 使地址簿全部 IPv6 候选在本地即拒（invalid remote address），直连全灭。
    pub fn new() -> io::Result<Self> {
        match Self::dual_stack_endpoint() {
            Ok(endpoint) => {
                tracing::info!(bind = "[::]:0", "quic dial endpoint dual-stack");
                Ok(Self {
                    endpoint,
                    dual_stack: true,
                })
            }
            Err(v6_err) => {
                tracing::warn!(error = %v6_err, "host without IPv6; quic dial endpoint falls back to IPv4-only");
                let endpoint =
                    quinn::Endpoint::client(SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 0))?;
                Ok(Self {
                    endpoint,
                    dual_stack: false,
                })
            }
        }
    }

    /// socket2 建 IPV6_V6ONLY=false 的 UDP socket 交 quinn 作底层（社区标准
    /// 双栈做法）：set_only_v6 必须先于 bind，Windows 同样要求。
    fn dual_stack_endpoint() -> io::Result<quinn::Endpoint> {
        let sock = Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))?;
        sock.set_only_v6(false)?;
        sock.bind(&SocketAddr::new(IpAddr::from([0u16; 8]), 0).into())?;
        sock.set_nonblocking(true)?;
        let std_sock: std::net::UdpSocket = sock.into();
        quinn::Endpoint::new(
            quinn::EndpointConfig::default(),
            None,
            std_sock,
            Arc::new(quinn::TokioRuntime),
        )
        .map_err(|e| io::Error::other(e.to_string()))
    }

    /// 监听端：以给定身份接受入站 QUIC。
    pub async fn bind(addr: SocketAddr, keypair: &Keypair) -> io::Result<Self> {
        // E7-K2：内层错误经 ChainedPayload 装箱，source() 遍历可达 SecurityError
        let crypto = quic_server_config(keypair).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                crate::ChainedPayload { inner: e },
            )
        })?;
        let quic_crypto = QuicServerConfig::try_from(Arc::new(crypto))
            .map_err(|e| io::Error::other(crate::ChainedPayload { inner: e }))?;
        let mut server = quinn::ServerConfig::with_crypto(Arc::new(quic_crypto));
        server.transport_config(stream_limits());
        let endpoint = quinn::Endpoint::server(server, addr)?;
        // 监听端点不是拨号出口（swarm 拨号恒走 new() 端点），双栈映射不适用
        Ok(Self {
            endpoint,
            dual_stack: false,
        })
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
        // E7-K2：TLS 配置失败的内层错误整体挂 source 链
        let crypto = quic_client_config(keypair).map_err(|e| TransportError::HandshakeChained {
            source: Box::new(e),
        })?;
        let quic_crypto = QuicClientConfig::try_from(Arc::new(crypto)).map_err(|e| {
            TransportError::HandshakeChained {
                source: Box::new(e),
            }
        })?;
        let mut client = quinn::ClientConfig::new(Arc::new(quic_crypto));
        let mut transport = quinn::TransportConfig::default();
        transport.keep_alive_interval(Some(KEEP_ALIVE));
        transport.max_idle_timeout(Some(
            quinn::IdleTimeout::try_from(QUIC_IDLE_TIMEOUT).expect("idle timeout fits varint"),
        ));
        client.transport_config(Arc::new(transport));

        let connect_addr = connect_target(addr, ip, port, self.dual_stack)?;
        let connecting = self
            .endpoint
            .connect_with(client, connect_addr, SERVER_NAME)
            .map_err(|e| TransportError::DialChained {
                addr: addr.to_string(),
                source: Box::new(e),
            })?;
        let conn = match tokio::time::timeout(HANDSHAKE_TIMEOUT, connecting).await {
            Ok(Ok(conn)) => conn,
            // E5 关联面：握手期连接错误（quinn::ConnectionError）整体挂 source 链
            Ok(Err(e)) => {
                return Err(TransportError::DialChained {
                    addr: addr.to_string(),
                    source: Box::new(e),
                });
            }
            Err(_) => {
                return Err(crate::dial_timeout(
                    addr,
                    HANDSHAKE_TIMEOUT,
                    "quic handshake",
                ));
            }
        };
        secure_conn(conn, expected)
    }
}
