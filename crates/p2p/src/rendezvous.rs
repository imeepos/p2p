//! rendezvous 接线（design §2/§5.4/§7.2）：
//! 客户端 RendezvousLink 拨号实现 + 服务端内置 handler（与业务协议同机制注册）。

use std::io;
use std::net::IpAddr;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use p2p_discovery::rendezvous::{serve_link, RendezvousConn, RendezvousError, RendezvousLink};
use p2p_discovery::RendezvousRegistry;
use p2p_identity::Keypair;
use p2p_mux::BoxedStream;
use p2p_protocol::{open_with_protocol, ProtocolHandler, ProtocolId};
use p2p_transport::{QuicTransport, SecureConn, TcpTransport, Transport, TransportAddr};
use tokio::sync::mpsc;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::NodeError;

/// 内置 rendezvous 控制协议（design §5.4）。
const RENDEZVOUS_PROTOCOL: &str = "/p2p-base/rendezvous/1";

/// 解析 `ip/u端口`（QUIC）或 `ip/t端口`（TCP），与 TransportAddr 展示格式一致。
pub(crate) fn parse_transport_addr(s: &str) -> Result<TransportAddr, NodeError> {
    let bad = || NodeError::Assembly(format!("bad transport addr: {s}"));
    let (ip_str, tail) = s.split_once('/').ok_or_else(bad)?;
    let ip: IpAddr = ip_str.parse().map_err(|_| bad())?;
    let mut rest = tail.chars();
    let kind = rest.next().ok_or_else(bad)?;
    let port: u16 = rest.as_str().parse().map_err(|_| bad())?;
    match kind {
        'u' => Ok(TransportAddr::Quic { ip, port }),
        't' => Ok(TransportAddr::Tcp { ip, port }),
        _ => Err(bad()),
    }
}

/// 客户端连接缝：拨号 bootstrap → 开流 → 协议握手 → 长度分帧。
pub(crate) struct TransportLink {
    addrs: Vec<TransportAddr>,
    keypair: Arc<Keypair>,
    quic: QuicTransport,
    tcp: TcpTransport,
}

impl TransportLink {
    pub(crate) fn new(addrs: Vec<TransportAddr>, keypair: Arc<Keypair>) -> io::Result<Self> {
        Ok(Self {
            addrs,
            keypair,
            quic: QuicTransport::new()?,
            tcp: TcpTransport::new(),
        })
    }
}

#[async_trait]
impl RendezvousLink for TransportLink {
    async fn connect(&self) -> Result<RendezvousConn, RendezvousError> {
        let mut last: Option<RendezvousError> = None;
        for addr in &self.addrs {
            let transport: &dyn Transport = match addr {
                TransportAddr::Quic { .. } => &self.quic,
                TransportAddr::Tcp { .. } => &self.tcp,
            };
            // 盲拨例外：bootstrap 仅以地址配置、身份未知，无法比对 expected；
            // 依赖与 bootstrap 间加密信道（security-review-1.md L1），显式留痕
            tracing::warn!(%addr, "rendezvous blind dial: bootstrap peer unknown, no expected binding");
            let conn = match transport.dial(addr, &self.keypair, None).await {
                Ok(conn) => conn,
                Err(e) => {
                    last = Some(RendezvousError::Link(e.to_string()));
                    continue;
                }
            };
            match open_rendezvous_stream(&conn).await {
                Ok(stream) => return Ok(stream_to_conn(stream)),
                Err(e) => last = Some(RendezvousError::Link(e.to_string())),
            }
        }
        Err(last.unwrap_or_else(|| RendezvousError::Link("no bootstrap addrs".into())))
    }
}

async fn open_rendezvous_stream(conn: &SecureConn) -> io::Result<BoxedStream> {
    let id = ProtocolId::new(RENDEZVOUS_PROTOCOL).expect("built-in protocol id is valid");
    let raw = conn.mux.open_stream().await?;
    open_with_protocol(raw, &id).await
}

/// BoxedStream → 长度分帧 RendezvousConn（与服务端 serve_link 帧约定一致）。
pub(crate) fn stream_to_conn(stream: BoxedStream) -> RendezvousConn {
    let (rx, tx) = tokio::io::split(stream);
    let (out_tx, mut out_rx) = mpsc::channel::<Vec<u8>>(16);
    tokio::spawn(async move {
        let mut framed = FramedWrite::new(tx, LengthDelimitedCodec::new());
        while let Some(frame) = out_rx.recv().await {
            if framed.send(Bytes::from(frame)).await.is_err() {
                break;
            }
        }
        let _ = framed.close().await;
    });
    let (in_tx, in_rx) = mpsc::channel::<Result<Vec<u8>, RendezvousError>>(16);
    tokio::spawn(async move {
        let mut framed = FramedRead::new(rx, LengthDelimitedCodec::new());
        while let Some(item) = framed.next().await {
            let msg = item
                .map_err(|e| RendezvousError::Link(e.to_string()))
                .map(|frame| frame.to_vec());
            if in_tx.send(msg).await.is_err() {
                break;
            }
        }
    });
    let read = Box::pin(futures::stream::unfold(in_rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    }));
    RendezvousConn {
        write: out_tx,
        read,
    }
}

/// 内置 rendezvous 服务 handler：入站流桥接到 serve_link（design §2/§5.4）。
pub(crate) struct RendezvousServer {
    registry: Arc<RendezvousRegistry>,
}

impl RendezvousServer {
    pub(crate) fn new() -> Self {
        Self {
            registry: Arc::new(RendezvousRegistry::new()),
        }
    }
}

#[async_trait]
impl ProtocolHandler for RendezvousServer {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(RENDEZVOUS_PROTOCOL).expect("built-in protocol id is valid")
    }

    async fn handle(&self, stream: BoxedStream) -> io::Result<()> {
        let mut conn = stream_to_conn(stream);
        if let Err(e) = serve_link(&mut conn, &self.registry).await {
            tracing::warn!(error = %e, "rendezvous server link ended");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_transport_addr_roundtrip() {
        let quic = parse_transport_addr("192.168.1.5/u40000").expect("quic addr");
        assert_eq!(
            quic,
            TransportAddr::Quic {
                ip: "192.168.1.5".parse().unwrap(),
                port: 40000
            }
        );
        let tcp = parse_transport_addr("127.0.0.1/t40001").expect("tcp addr");
        assert_eq!(
            tcp,
            TransportAddr::Tcp {
                ip: "127.0.0.1".parse().unwrap(),
                port: 40001
            }
        );
        assert!(parse_transport_addr("127.0.0.1/x1").is_err());
        assert!(parse_transport_addr("127.0.0.1/u").is_err());
        assert!(parse_transport_addr("no-slash/u1").is_err());
        assert!(parse_transport_addr("bad-ip/u1").is_err());
    }
}
