//! p2p 端点：受理 /repair/mcp/1 入站流（T26，remote-support-plan.md §3.1）。
//!
//! 组装：helper 经底座 facade 起节点（身份密钥经 data_dir 参数化）、向
//! rendezvous 注册（bootstrap 接线）；受理前置 = 票据校验全过（§3.3）。
//! 冻结接缝约束：ProtocolHandler::handle 只给流、不携带对端身份，入流对端
//! 经连接门禁（ConnectionGate）在连接建立期捕获（listen.rs 同源裁决）。
//!
//! 断线语义（§3.7）：帧泵任一方向终止（对端断流/写侧失败）即结束受理——
//! Host::serve 的 reader 随 in_tx 关闭收到 EOF 自然收尾；挂起中的审批视同
//! 拒绝（T23b：审批状态机 60s 超时即拒，断线无人应答自然超时，无放行路径）。
//! shell_exec 在 fix scope 走工具内审批（approval 通道经 Endpoint 注入）。

use std::io;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use p2p::{gate_fn, BoxedStream, ConnectionGate, ProtocolHandler, ProtocolId};
use p2p_identity::PeerId;
use p2p_protocol::{read_frame, write_frame};
use repair_bridge::PROTOCOL_ID;
use repair_enforce::approval::{Approver, Clock};
use repair_enforce::{scope::Scope, whitelist::ShellWhitelist};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, DuplexStream};
use tokio::sync::watch;

use crate::audit::AuditSink;
use crate::enforce::Enforcement;
use crate::jail::PathJail;
use crate::session_report::SessionReport;
use crate::ticket::{TicketPayload, TicketVerifier, SCOPE_DIAG};
use crate::tools::{self, shell_exec::ShellExec};
use crate::Host;

/// 帧泵双工的缓冲字节数（Host 行读写与桥帧速率的背压缓冲）。
const DUPLEX_CAPACITY: usize = 1 << 20;

/// 出站字节泵单次读取上限（读到的字节按 MAX_FRAME_SIZE 分帧）。
const OUT_PUMP_CHUNK: usize = 64 * 1024;

/// 入流对端观测：连接门禁捕获的最近入站 peer（P0b 单桥场景）。
#[derive(Clone, Default)]
pub struct InboundPeers {
    inner: Arc<Mutex<Option<PeerId>>>,
}

impl InboundPeers {
    /// 门禁回调解用：记录入站连接对端；锁中毒拒绝该连接（显式，不静默放行）。
    pub fn record(&self, peer: PeerId) -> bool {
        match self.inner.lock() {
            Ok(mut slot) => {
                *slot = Some(peer);
                true
            }
            Err(_) => {
                tracing::error!("inbound peers lock poisoned; connection denied");
                false
            }
        }
    }

    /// 最近一次入站连接对端（无入站记录为 None）。
    pub fn last(&self) -> Option<PeerId> {
        self.inner.lock().ok().and_then(|slot| *slot)
    }

    /// 连接门禁（frozen seam）：放行一切连接并在门禁层记录其 peer。
    pub fn gate(&self) -> Arc<dyn ConnectionGate> {
        let this = self.clone();
        Arc::new(gate_fn(move |peer| this.record(*peer)))
    }
}

/// /repair/mcp/1 入站流处理器：票据校验 -> scope 装配 -> guarded Host 承接。
pub struct Endpoint {
    verifier: TicketVerifier,
    peers: InboundPeers,
    jail: PathJail,
    audit: AuditSink,
    whitelist: ShellWhitelist,
    clock: Arc<dyn Clock + Send + Sync>,
    approver: Arc<Mutex<Box<dyn Approver + Send>>>,
    protocol: ProtocolId,
}

impl Endpoint {
    /// 协议 ID 在装配期校验（常量非法即启动失败，禁 panic）。
    pub fn new(
        verifier: TicketVerifier,
        peers: InboundPeers,
        jail: PathJail,
        audit: AuditSink,
        whitelist: ShellWhitelist,
        clock: Arc<dyn Clock + Send + Sync>,
        approver: Arc<Mutex<Box<dyn Approver + Send>>>,
    ) -> Result<Self, io::Error> {
        let protocol = ProtocolId::new(PROTOCOL_ID)
            .map_err(|e| io::Error::other(format!("protocol id {PROTOCOL_ID}: {e}")))?;
        Ok(Self {
            verifier,
            peers,
            jail,
            audit,
            whitelist,
            clock,
            approver,
            protocol,
        })
    }
}

#[async_trait]
impl ProtocolHandler for Endpoint {
    fn protocol(&self) -> ProtocolId {
        self.protocol.clone()
    }

    async fn handle(&self, stream: BoxedStream) -> io::Result<()> {
        let mut stream = stream;
        // 首帧 = 票据（bridge 开流后先发票据再泵 MCP 字节，见 repair-bridge）。
        let ticket_bytes = read_frame(&mut stream).await?;
        let ticket = String::from_utf8(ticket_bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "ticket not utf-8"))?;
        let peer = self.peers.last().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotConnected,
                "no inbound peer observed by connection gate",
            )
        })?;
        let now_unix = crate::audit::now_unix_ms() / 1000;
        let payload = match self.verifier.verify(&ticket, &peer, now_unix) {
            Ok(payload) => payload,
            Err(reason) => {
                tracing::warn!(%reason, %peer, "repair stream rejected (ticket not accepted)");
                return Err(io::Error::other(reason.to_string()));
            }
        };
        let host = self.host_for(&payload);
        tracing::info!(
            ticket_id = %payload.ticket_id,
            scope = %payload.scope,
            %peer,
            "repair stream accepted"
        );
        serve_framed(host, stream).await
    }
}

impl Endpoint {
    /// 按票据装配本例 Host：scope 来自 ticket（diag/fix），审计/工具注册表共享。
    fn host_for(&self, payload: &TicketPayload) -> Host {
        let scope = match payload.scope.as_str() {
            SCOPE_DIAG => Scope::Diag,
            _ => Scope::Fix,
        };
        let mut registry = tools::read_only_registry(self.jail.clone());
        let enforcement = Enforcement::new(scope, self.whitelist.clone());
        registry.register(ShellExec::new(
            self.jail.clone(),
            enforcement.clone(),
            self.clock.clone(),
            self.approver.clone(),
        ));
        registry.register(SessionReport::new(
            self.audit.clone(),
            payload.ticket_id.clone(),
        ));
        Host::guarded(registry, enforcement, self.audit.clone())
    }
}

/// 双工适配：帧流 <-> 字节流（Host::serve 需行分隔字节流，桥以帧承载 MCP 字节）。
/// 任一方向终止（含断线）即结束本次受理（§3.7）。
async fn serve_framed(host: Host, stream: BoxedStream) -> io::Result<()> {
    let (stream_r, stream_w) = tokio::io::split(stream);
    let (in_tx, in_rx) = tokio::io::duplex(DUPLEX_CAPACITY);
    let (out_tx, out_rx) = tokio::io::duplex(DUPLEX_CAPACITY);
    let mut recv = tokio::spawn(frame_to_bytes(stream_r, in_tx));
    let mut send = tokio::spawn(bytes_to_frame(out_rx, stream_w));
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    // serve 错误内联记录（禁止吞错）；select 消费过的句柄不再二次 await。
    let mut serve_task = tokio::spawn(async move {
        if let Err(error) = host.serve(BufReader::new(in_rx), out_tx, shutdown_rx).await {
            tracing::debug!(%error, "mcp serve ended with error after pump stop");
        }
    });
    tokio::select! {
        _ = &mut serve_task => { recv.abort(); send.abort(); }
        _ = &mut recv => { send.abort(); }
        _ = &mut send => { recv.abort(); }
    }
    drop(serve_task);
    drop(recv);
    drop(send);
    Ok(())
}

/// 入站：帧 -> 字节写入 in_tx（Host reader）；流断即停止并关半（EOF 语义）。
async fn frame_to_bytes<R>(mut stream: R, mut tx: DuplexStream) -> io::Result<()>
where
    R: AsyncRead + Unpin + Send,
{
    loop {
        let frame = match read_frame(&mut stream).await {
            Ok(frame) => frame,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(error) => {
                tracing::warn!(%error, "inbound frame stream error; closing");
                break;
            }
        };
        if tx.write_all(&frame).await.is_err() {
            break;
        }
    }
    let _ = tx.shutdown().await;
    Ok(())
}

/// 出站：字节（Host 写侧）-> 按 MAX_FRAME_SIZE 分帧写回桥；写侧失败留日志。
async fn bytes_to_frame<W>(mut rx: DuplexStream, mut stream: W) -> io::Result<()>
where
    W: AsyncWrite + Unpin + Send,
{
    let mut buf = vec![0u8; OUT_PUMP_CHUNK];
    loop {
        let n = match rx.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(error) => {
                tracing::debug!(%error, "outbound byte stream closed");
                break;
            }
        };
        for chunk in buf[..n].chunks(p2p_protocol::MAX_FRAME_SIZE as usize) {
            write_frame(&mut stream, chunk).await?;
        }
        if let Err(error) = stream.flush().await {
            tracing::warn!(%error, "outbound frame flush failed");
            break;
        }
    }
    Ok(())
}
