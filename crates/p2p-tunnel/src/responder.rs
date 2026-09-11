//! /p2p-base/tunnel/1 被访侧（契约 §1-§5）：读票据帧 → 校验（结构/版本/ts/
//! 回环形态）→ 准入（开关/白名单/并发）→ 拨本地目标 → ack → 双向哑泵 → 审计。
//! 拒绝路径一律显式回 error 帧并落审计（契约 §3：禁止静默断流）。

use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{read_frame, write_frame, ProtocolHandler, ProtocolId};
use tokio::time::timeout_at;

use crate::audit::{TunnelAudit, TunnelAuditOutcome, TunnelAuditRecord};
use crate::config::{TunnelGate, TunnelServeConfig};
use crate::error::TunnelError;
use crate::pump::tunnel_pump;
use crate::wire::{
    is_loopback_literal_target, now_unix_secs, TunnelErrorCode, TunnelReply, TunnelTicket,
    MAX_TICKET_BYTES, PROTOCOL_ID,
};
use crate::TunnelIo;

/// 本地目标拨号缝：被访侧把隧道流接到 `127.0.0.1:<port>` 的本地服务上。
#[async_trait]
pub trait HttpDialer: Send + Sync {
    async fn dial(&self, target: &str) -> Result<TunnelIo, TunnelError>;
}

/// 真实拨号器：仅回环字面量目标（白名单之外的第二道硬拦）。
#[derive(Debug, Default, Clone, Copy)]
pub struct TcpDialer;

#[async_trait]
impl HttpDialer for TcpDialer {
    async fn dial(&self, target: &str) -> Result<TunnelIo, TunnelError> {
        if !is_loopback_literal_target(target) {
            return Err(TunnelError::Rejected {
                code: TunnelErrorCode::TargetNotAllowed,
                message: format!("non-loopback dial target: {target}"),
            });
        }
        Ok(Box::new(tokio::net::TcpStream::connect(target).await?))
    }
}

/// 被访侧协议 handler：注册到 Node::handle_protocol 即服务该协议。
pub struct TunnelResponder<H: HttpDialer> {
    protocol: ProtocolId,
    gate: TunnelGate,
    dialer: Arc<H>,
    audit: Arc<TunnelAudit>,
}

impl<H: HttpDialer> TunnelResponder<H> {
    pub fn new(protocol: ProtocolId, gate: TunnelGate, dialer: H) -> Self {
        Self {
            protocol,
            gate,
            dialer: Arc::new(dialer),
            audit: Arc::new(TunnelAudit::new()),
        }
    }

    /// 审计账句柄（产品层/测试读快照）。
    pub fn audit(&self) -> Arc<TunnelAudit> {
        self.audit.clone()
    }

    /// 单会话服务（swarm 分发入口）。返回即会话已收口（含拒绝路径）。
    pub async fn serve_session(&self, peer: PeerId, mut stream: BoxedStream) -> io::Result<()> {
        let cfg = self.gate.cfg();
        let started_at = now_unix_secs();
        let session = match self.read_ticket(&mut stream, &cfg).await {
            Ok(ticket) => ticket,
            Err(reject) => {
                self.reject(peer, &mut stream, started_at, reject).await;
                return Ok(());
            }
        };
        self.admit_and_pump(peer, stream, session, started_at, cfg)
            .await;
        Ok(())
    }

    /// 准入顺序（fail-closed）：开关 → 白名单 → 并发 → 拨号 → ack → 哑泵。
    async fn admit_and_pump(
        &self,
        peer: PeerId,
        mut stream: BoxedStream,
        session: TunnelTicket,
        started_at: u64,
        cfg: Arc<TunnelServeConfig>,
    ) {
        let permit = match self.gate.authorize(&session.target) {
            Ok(permit) => permit,
            Err(code) => {
                let message = format!("gate: {code}");
                self.reject(
                    peer,
                    &mut stream,
                    started_at,
                    Reject::new(code, message).with_ticket(session.clone()),
                )
                .await;
                return;
            }
        };
        let io = match self.dialer.dial(&session.target).await {
            Ok(io) => io,
            Err(e) => {
                let message = format!("dial {}: {e}", session.target);
                self.reject(
                    peer,
                    &mut stream,
                    started_at,
                    Reject::new(TunnelErrorCode::DialFailed, message).with_ticket(session.clone()),
                )
                .await;
                return;
            }
        };
        let _permit = permit; // 持有期 = 会话期
        let ack = match TunnelReply::ack(&session.uid).encode() {
            Ok(ack) => ack,
            Err(e) => {
                tracing::error!(session_id = %session.uid, error = %e, "ack encode failed");
                return;
            }
        };
        if write_frame(&mut stream, &ack).await.is_err() {
            tracing::warn!(session_id = %session.uid, "ack write failed, peer gone");
            return;
        }
        let result = tunnel_pump(stream, io, cfg.chunk_size, cfg.session_timeout).await;
        let outcome = match &result.error {
            None => TunnelAuditOutcome::Served,
            Some(error) => {
                tracing::warn!(session_id = %session.uid, error = %error, "tunnel broken");
                TunnelAuditOutcome::Broken(TunnelErrorCode::Io)
            }
        };
        self.audit.record(TunnelAuditRecord {
            session_id: session.uid.clone(),
            peer_id: peer.to_string(),
            target: session.target.clone(),
            started_at,
            ended_at: now_unix_secs(),
            bytes_in: result.totals.bytes_in,
            bytes_out: result.totals.bytes_out,
            outcome,
        });
    }

    /// 首帧票据（契约 §1/§2）：≤4 KiB、5s 内、结构与策略合法。
    ///
    /// Node facade 双写握手容差（llm-share-proxy wire.rs 同款先例）：factory
    /// （Node::new_stream）与 client（open_with_protocol）各写一帧协议 ID，swarm
    /// 分发消费一帧后 handler 流上仍余一帧；识别即校验并跳过，其余形态按票据读。
    async fn read_ticket(
        &self,
        stream: &mut BoxedStream,
        cfg: &TunnelServeConfig,
    ) -> Result<TunnelTicket, Reject> {
        let deadline = tokio::time::Instant::now() + cfg.ticket_timeout;
        let raw = loop {
            let frame = timeout_at(deadline, read_frame(stream))
                .await
                .map_err(|_| {
                    Reject::new(
                        TunnelErrorCode::BadTicket,
                        format!("no ticket within {:?}", cfg.ticket_timeout),
                    )
                })?
                .map_err(|e| {
                    Reject::new(
                        TunnelErrorCode::BadTicket,
                        format!("ticket frame unreadable: {e}"),
                    )
                })?;
            match frame.first() {
                Some(b'/') => {
                    let id = String::from_utf8_lossy(&frame);
                    if id != PROTOCOL_ID {
                        return Err(Reject::new(
                            TunnelErrorCode::BadTicket,
                            format!("unexpected protocol {id}"),
                        ));
                    }
                }
                _ => break frame,
            }
        };
        if raw.len() > MAX_TICKET_BYTES {
            return Err(Reject::new(
                TunnelErrorCode::BadTicket,
                format!(
                    "ticket frame {} bytes exceeds {MAX_TICKET_BYTES}",
                    raw.len()
                ),
            ));
        }
        let ticket = TunnelTicket::decode(&raw)
            .map_err(|e| Reject::new(TunnelErrorCode::BadTicket, format!("ticket decode: {e}")))?;
        ticket
            .validate(now_unix_secs(), cfg.ts_window_secs)
            .map_err(|code| {
                Reject::new(code, format!("ticket policy: {code}")).with_ticket(ticket.clone())
            })?;
        Ok(ticket)
    }

    /// 拒绝收口：显式 error 帧（尽力送达）+ 审计留痕。
    async fn reject(
        &self,
        peer: PeerId,
        stream: &mut BoxedStream,
        started_at: u64,
        reject: Reject,
    ) {
        let Reject {
            code,
            message,
            ticket,
        } = reject;
        match TunnelReply::error(code, message.clone()).encode() {
            Ok(frame) => {
                if let Err(e) = write_frame(stream, &frame).await {
                    tracing::warn!(%code, peer = %peer, error = %e, "error frame write failed");
                }
            }
            Err(e) => tracing::error!(%code, error = %e, "error frame encode failed"),
        }
        let (session_id, target) = match &ticket {
            Some(t) => (t.uid.clone(), t.target.clone()),
            None => ("-".to_string(), "-".to_string()),
        };
        self.audit.record(TunnelAuditRecord {
            session_id,
            peer_id: peer.to_string(),
            target,
            started_at,
            ended_at: now_unix_secs(),
            bytes_in: 0,
            bytes_out: 0,
            outcome: TunnelAuditOutcome::Rejected(code),
        });
    }
}

/// 拒绝上下文：错误码 + 人读原因 +（可解码时的）票据供审计归档。
struct Reject {
    code: TunnelErrorCode,
    message: String,
    ticket: Option<TunnelTicket>,
}

impl Reject {
    fn new(code: TunnelErrorCode, message: String) -> Self {
        Self {
            code,
            message,
            ticket: None,
        }
    }

    fn with_ticket(mut self, ticket: TunnelTicket) -> Self {
        self.ticket = Some(ticket);
        self
    }
}

#[async_trait]
impl<H: HttpDialer> ProtocolHandler for TunnelResponder<H> {
    fn protocol(&self) -> ProtocolId {
        self.protocol.clone()
    }

    /// 身份取底座安全握手 PeerId（契约 §2：不采信票据自报身份）。
    async fn handle_inbound(&self, peer: PeerId, stream: BoxedStream) -> io::Result<()> {
        self.serve_session(peer, stream).await
    }

    /// 裸流无认证身份上下文：审计归属必须有真实对端，fail-closed。
    async fn handle(&self, _stream: BoxedStream) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "tunnel requires authenticated peer identity",
        ))
    }
}

#[cfg(test)]
#[path = "responder_tests.rs"]
mod tests;
