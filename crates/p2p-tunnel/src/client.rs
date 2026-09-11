//! 访侧开流客户端（W-T3 直接消费）：开流（协议 ID 首帧）→ 票据帧 → 应答帧；
//! ack 后交付裸双向字节 IO（半关 = AsyncWrite::shutdown），error 帧结构化上抛。

use std::io;
use std::time::Duration;

use p2p_identity::PeerId;
use p2p_protocol::{open_with_protocol, read_frame, write_frame, StreamFactory};
use tokio::time::timeout;

use crate::error::TunnelError;
use crate::pump::tunnel_pump;
use crate::wire::{TunnelReply, TunnelTicket};
use crate::{protocol_id, TunnelIo};

/// 访侧泵的出站分块上限（与被访侧默认一致，契约 §1）。
const CLIENT_CHUNK: usize = 64 * 1024;

/// 访侧隧道客户端：Clone 共享工厂。
#[derive(Clone)]
pub struct TunnelClient<S: StreamFactory> {
    factory: S,
    reply_timeout: Duration,
}

impl<S: StreamFactory> TunnelClient<S> {
    pub fn new(factory: S) -> Self {
        Self {
            factory,
            reply_timeout: Duration::from_secs(10),
        }
    }

    /// 应答帧等待上限（仅建流期，不含数据面）。
    pub fn with_reply_timeout(mut self, timeout: Duration) -> Self {
        self.reply_timeout = timeout;
        self
    }

    /// `open(peer, ticket) -> TunnelIo`（契约 §7）：成功即 ack 已收，进入字节流阶段。
    pub async fn open(
        &self,
        peer: PeerId,
        ticket: &TunnelTicket,
    ) -> Result<TunnelIo, TunnelError> {
        let id = protocol_id().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let opened = self.factory.open_stream(&peer, &id).await?;
        let mut stream = open_with_protocol(opened, &id).await?;
        let ticket_frame = ticket
            .encode()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        write_frame(&mut stream, &ticket_frame).await?;
        let raw = match timeout(self.reply_timeout, read_frame(&mut stream)).await {
            Ok(raw) => raw?,
            Err(_) => {
                return Err(TunnelError::Io(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("tunnel reply not received within {:?}", self.reply_timeout),
                )))
            }
        };
        let reply = TunnelReply::decode(&raw)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        match reply {
            TunnelReply::Ack { uid } if uid == ticket.uid => {
                // 访侧泵内置（wired tunnel_pump）：返回的 TunnelIo 已是裸字节，
                // 帧封装在此终结/重建，W-T3 反代可直接对它做流式 copy；半关语义
                // 原样透传（shutdown → FIN）。
                let (mine, theirs) = tokio::io::duplex(CLIENT_CHUNK);
                let wire: TunnelIo = Box::new(stream);
                let raw: TunnelIo = Box::new(theirs);
                tokio::spawn(tunnel_pump(wire, raw, CLIENT_CHUNK, None));
                Ok(Box::new(mine))
            }
            TunnelReply::Ack { uid } => Err(TunnelError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("ack uid mismatch: {uid} != {}", ticket.uid),
            ))),
            TunnelReply::Error { code, msg } => Err(TunnelError::Rejected {
                code,
                message: msg.unwrap_or_default(),
            }),
        }
    }
}
