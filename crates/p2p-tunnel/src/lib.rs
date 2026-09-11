//! /p2p-base/tunnel/1 点对点本地端口隧道（W-T2）：
//! 被访侧 [TunnelResponder]（票据→准入→拨本地→哑泵→审计）+ 访侧 [TunnelClient]。
//!
//! 冻结导出面（契约 §7，W-T3 直接消费）：PROTOCOL_ID / TunnelTicket /
//! TunnelErrorCode / TunnelResponder / HttpDialer / TunnelClient / TunnelIo。
//! HTTP 语义不进本 crate（访侧本地反代与 Host 重写属 W-T3 域）。

mod audit;
mod client;
mod config;
mod error;
mod pump;
mod responder;
mod wire;

pub use audit::{TunnelAudit, TunnelAuditOutcome, TunnelAuditRecord};
pub use client::TunnelClient;
pub use config::{GateStatus, TunnelGate, TunnelPermit, TunnelServeConfig};
pub use error::TunnelError;
pub use pump::{tunnel_pump, PumpResult, PumpTotals};
pub use responder::{HttpDialer, TcpDialer, TunnelResponder};
pub use wire::{
    now_unix_secs, TunnelErrorCode, TunnelReply, TunnelTicket, MAX_TICKET_BYTES, PROTOCOL_ID,
    TS_WINDOW_SECS,
};

use tokio::io::{AsyncRead, AsyncWrite};

/// 隧道数据面字节流统一约束（p2p_mux::ByteStream 同款形态，对象安全）。
pub trait TunnelStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> TunnelStream for T {}

/// 隧道数据面交付物：裸双向字节流；`finish()` = [AsyncWrite::shutdown]
/// （本方向半关，对端读 EOF，契约 §4）。
pub type TunnelIo = Box<dyn TunnelStream>;

/// 协议 ID 唯一定义点（契约 §1）：由 [PROTOCOL_ID] 常量解析。
pub fn protocol_id() -> Result<p2p_protocol::ProtocolId, p2p_protocol::ProtocolError> {
    p2p_protocol::ProtocolId::new(PROTOCOL_ID)
}
