//! /p2p-base/tunnel/1 点对点本地端口隧道（W-T2）：
//! 被访侧 [TunnelResponder]（票据→准入→拨本地→哑泵→审计）+ 访侧 [TunnelClient]。
//!
//! 冻结导出面（契约 §7）：PROTOCOL_ID / TunnelTicket / TunnelErrorCode /
//! TunnelResponder / HttpDialer / TunnelClient / TunnelIo / local_proxy 族。
//! 访侧本地反代核心（LocalProxy/头重写/数据泵）落 local_proxy 模块（W-TA
//! 契约先行签名桩，todo!("W-TB")；实现填埋与 GUI 切换 = W-TB；HTTP 语义
//! 自本模块起进本 crate，冻结接口表 LocalProxy 行归属此处）。

mod audit;
mod client;
mod config;
mod error;
mod local_proxy;
mod pump;
mod responder;
mod wire;

pub use audit::{TunnelAudit, TunnelAuditOutcome, TunnelAuditRecord};
pub use client::TunnelClient;
pub use config::{GateStatus, TunnelGate, TunnelPermit, TunnelServeConfig};
pub use error::TunnelError;
pub use local_proxy::head::{read_head, Head, HEAD_MAX};
pub use local_proxy::pump::{
    drain_reply, forward_exact, is_101, read_reply_head, PumpAudit, TUNNEL_IDLE_GRACE,
};
pub use local_proxy::{LocalProxy, ProxyCtx, TunnelOpener};
pub use pump::{tunnel_pump, PumpResult, PumpTotals};
pub use responder::{HttpDialer, TcpDialer, TunnelResponder};
pub use wire::{
    is_loopback_literal_target, now_unix_secs, TunnelErrorCode, TunnelReply, TunnelTicket,
    MAX_TICKET_BYTES, PROTOCOL_ID, TS_WINDOW_SECS,
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
