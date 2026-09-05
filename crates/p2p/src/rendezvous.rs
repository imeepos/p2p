//! rendezvous 接线（design §2/§5.4/§7.2）：
//! 客户端 RendezvousLink 拨号实现 + 服务端内置 handler（与业务协议同机制注册）。

use std::io;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use p2p_discovery::rendezvous::{serve_link, RendezvousConn, RendezvousError, RendezvousLink};
use p2p_discovery::RendezvousRegistry;
use p2p_identity::Keypair;
use p2p_mux::BoxedStream;
use p2p_protocol::{
    dispatch_inbound, open_with_protocol, HandlerRegistry, ProtocolHandler, ProtocolId,
};
use p2p_swarm::PingHandler;
use p2p_transport::{QuicTransport, SecureConn, TcpTransport, Transport, TransportAddr};
use tokio::sync::mpsc;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::discovery::failure_notice_level;
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
/// 导出给 itest 驱动生产盲拨路径（facade 服务端 ↔ 裸链客户端）。
pub struct TransportLink {
    addrs: Vec<TransportAddr>,
    /// 与 addrs 一一对应：该地址当前故障序列是否已 WARN（AtomicBool 适配 &self connect）。
    blind_dial_failed_warned: Vec<AtomicBool>,
    keypair: Arc<Keypair>,
    quic: QuicTransport,
    tcp: TcpTransport,
    /// 连接复用缓存（BASE1）：查号与周期注册/查询循环共用一条到 bootstrap
    /// 的连接——每查号新建连接既有握手开销，又会触发服务端同向重复入池
    /// 的收敛裁决与半开残留竞态窗口。缓存连接失效（开流失败）即剔除重拨。
    cached: tokio::sync::Mutex<Option<SecureConn>>,
}

impl TransportLink {
    pub fn new(addrs: Vec<TransportAddr>, keypair: Arc<Keypair>) -> io::Result<Self> {
        let warned = (0..addrs.len()).map(|_| AtomicBool::new(false)).collect();
        Ok(Self {
            addrs,
            blind_dial_failed_warned: warned,
            keypair,
            quic: QuicTransport::new()?,
            tcp: TcpTransport::new(),
            cached: tokio::sync::Mutex::new(None),
        })
    }

    /// 失败留痕（E4 刷屏治理，检查轮12）：单个 bootstrap 地址一次故障仅首次 WARN，
    /// 重连周期（退避上限 30s）重复失败降级 debug；成功后复位，下次故障重新告警。
    fn note_blind_dial_failure(
        &self,
        idx: usize,
        addr: &TransportAddr,
        err: &impl std::fmt::Display,
    ) {
        let already = self.blind_dial_failed_warned[idx].swap(true, Ordering::Relaxed);
        let level = failure_notice_level(already);
        if level == tracing::Level::WARN {
            tracing::warn!(
                %addr,
                error = %err,
                "rendezvous blind dial failed: bootstrap peer unknown, no expected binding"
            );
        } else {
            tracing::debug!(
                %addr,
                error = %err,
                "rendezvous blind dial failed: bootstrap peer unknown, no expected binding"
            );
        }
    }
}

#[async_trait]
impl RendezvousLink for TransportLink {
    async fn connect(&self) -> Result<RendezvousConn, RendezvousError> {
        // 连接复用：缓存连接仍可用（开流成功）即直接复用，不重拨
        if let Some(conn) = self.cached.lock().await.as_ref() {
            match open_rendezvous_stream(conn).await {
                Ok(stream) => {
                    tracing::debug!("rendezvous link reused cached bootstrap connection");
                    return Ok(stream_to_conn_owned(stream, clone_secure_conn(conn)));
                }
                Err(e) => {
                    tracing::debug!(error = %e, "cached bootstrap connection dead; redialing");
                    *self.cached.lock().await = None;
                }
            }
        }
        let (rendezvous_conn, secure) = self.dial_fresh().await?;
        *self.cached.lock().await = Some(secure);
        Ok(rendezvous_conn)
    }
}

impl TransportLink {
    /// 全新盲拨（遍历 bootstrap 地址）：成功时返回（rendezvous 会话, 连接句柄），
    /// 句柄由调用方入缓存以复用。
    async fn dial_fresh(&self) -> Result<(RendezvousConn, SecureConn), RendezvousError> {
        let mut last: Option<RendezvousError> = None;
        for (idx, addr) in self.addrs.iter().enumerate() {
            let transport: &dyn Transport = match addr {
                TransportAddr::Quic { .. } => &self.quic,
                TransportAddr::Tcp { .. } => &self.tcp,
            };
            // 盲拨例外：bootstrap 仅以地址配置、身份未知，无法比对 expected；
            // 依赖与 bootstrap 间加密信道（security-review-1.md L1），debug 留痕
            tracing::debug!(%addr, "rendezvous blind dial: bootstrap peer unknown, no expected binding");
            let conn = match transport.dial(addr, &self.keypair, None).await {
                Ok(conn) => conn,
                Err(e) => {
                    self.note_blind_dial_failure(idx, addr, &e);
                    last = Some(RendezvousError::Link(e.to_string()));
                    continue;
                }
            };
            match open_rendezvous_stream(&conn).await {
                Ok(stream) => {
                    self.blind_dial_failed_warned[idx].store(false, Ordering::Relaxed);
                    spawn_link_responder(&conn);
                    let secure = clone_secure_conn(&conn);
                    return Ok((stream_to_conn_owned(stream, conn), secure));
                }
                Err(e) => {
                    self.note_blind_dial_failure(idx, addr, &e);
                    last = Some(RendezvousError::Link(e.to_string()));
                }
            }
        }
        Err(last.unwrap_or_else(|| RendezvousError::Link("no bootstrap addrs".into())))
    }
}

/// SecureConn 不可 Clone，按字段克隆一份共享同一 mux 的句柄（缓存用）。
fn clone_secure_conn(conn: &SecureConn) -> SecureConn {
    SecureConn {
        remote: conn.remote,
        mux: Arc::clone(&conn.mux),
    }
}

/// 盲拨连接的入站应答循环：应答内置 ping（facade liveness 探针），未注册
/// 协议留 debug 日志后关流，不猜测降级。裸链不进 swarm，若无此循环对端探活
/// 永不命中，约 33s 即被判死掐线（RS 排障 2026-09-04，rendezvous_facade_link
/// itest 锚定：探活窗口生存契约）。
fn spawn_link_responder(conn: &SecureConn) {
    let mut handlers = HandlerRegistry::default();
    handlers.register(Arc::new(PingHandler));
    let handlers = Arc::new(handlers);
    let mux = conn.mux.clone();
    tokio::spawn(async move {
        while let Some(stream) = mux.accept_stream().await {
            if let Err(e) = dispatch_inbound(stream, &handlers).await {
                tracing::debug!(error = %e, "rendezvous link inbound stream ended");
            }
        }
        tracing::debug!("rendezvous link responder ended; connection closed");
    });
}

async fn open_rendezvous_stream(conn: &SecureConn) -> io::Result<BoxedStream> {
    // panic 免除红线（E8-H3）：构造失败走错误路径留信号，不 expect
    let id = builtin_rendezvous_id().map_err(io::Error::other)?;
    let raw = conn.mux.open_stream().await?;
    open_with_protocol(raw, &id).await
}

/// 内置 rendezvous 协议 ID 的唯一构造口：常量格式由 p2p-protocol 单测兜底，
/// 构造本身仍按可失败处理，装配期校验一次，运行期只读克隆。
pub(crate) fn builtin_rendezvous_id() -> Result<ProtocolId, NodeError> {
    ProtocolId::new(RENDEZVOUS_PROTOCOL)
        .map_err(|e| NodeError::Assembly(format!("builtin rendezvous protocol id invalid: {e}")))
}

/// BoxedStream → 长度分帧 RendezvousConn（与服务端 serve_link 帧约定一致）。
pub(crate) fn stream_to_conn(stream: BoxedStream) -> RendezvousConn {
    stream_to_conn_parts(stream, None)
}

fn stream_to_conn_owned(stream: BoxedStream, connection: SecureConn) -> RendezvousConn {
    stream_to_conn_parts(stream, Some(connection))
}

/// BoxedStream → 长度分帧 RendezvousConn（与服务端 serve_link 帧约定一致）。
/// 帧上限 1MiB（审查 M8：LengthDelimitedCodec 默认 8MiB 过宽，两端显式收口）。
pub(crate) const MAX_RENDEZVOUS_FRAME: usize = 1 << 20;

fn rendezvous_codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder()
        .max_frame_length(MAX_RENDEZVOUS_FRAME)
        .new_codec()
}

fn stream_to_conn_parts(stream: BoxedStream, connection: Option<SecureConn>) -> RendezvousConn {
    let (rx, tx) = tokio::io::split(stream);
    let (out_tx, mut out_rx) = mpsc::channel::<Vec<u8>>(16);
    tokio::spawn(async move {
        // TCP 的 YamuxMux 在所有句柄归零时关闭；持有连接直到 RendezvousConn 写端关闭。
        let _connection = connection;
        let mut framed = FramedWrite::new(tx, rendezvous_codec());
        while let Some(frame) = out_rx.recv().await {
            if framed.send(Bytes::from(frame)).await.is_err() {
                break;
            }
        }
        let _ = framed.close().await;
    });
    let (in_tx, in_rx) = mpsc::channel::<Result<Vec<u8>, RendezvousError>>(16);
    tokio::spawn(async move {
        let mut framed = FramedRead::new(rx, rendezvous_codec());
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
    id: ProtocolId,
    registry: Arc<RendezvousRegistry>,
}

impl RendezvousServer {
    /// 公共部署策略：拒收全 loopback/link-local 注册（E5 地址卫生，2026-09-03
    /// 邻居表 127.0.0.1 泄漏复盘）。CLI bootstrap 默认开启，--allow-private 退出。
    /// 协议 ID 在构造期校验：非法常量在装配期显式报错，而非 handler 分发时 panic。
    pub(crate) fn with_public_only(public_only: bool) -> Result<Self, NodeError> {
        Ok(Self {
            id: builtin_rendezvous_id()?,
            registry: Arc::new(RendezvousRegistry::with_public_only(public_only)),
        })
    }
}

#[async_trait]
impl ProtocolHandler for RendezvousServer {
    fn protocol(&self) -> ProtocolId {
        self.id.clone()
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
mod tests;
