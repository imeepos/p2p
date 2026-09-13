//! 入站准入闸（authz-role-design Amended A-1/A-2）：p2p-chat 只感知 trait，
//! 不感知 authz crate（§3 依赖红线）；实现方自带 reason 码与审计。
//! 判定次序（A-1）：先过 send 闸（media=false），消息携带媒体再过 attachment 闸
//! （media=true）；任一拒 = 整帧拒收，调用方不得落库/广播事件/回任何帧。
//! gate=None（缺省）= 不设防，与接线前行为逐字节一致（装配处 warn 可观测）。

use std::sync::Arc;

pub use p2p_identity::PeerId;

/// 入站准入拒绝；reason 为实现方语义码（chat 侧只断流记日志，不回 wire）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reject {
    pub reason: String,
}

/// 入站消息/媒体准入闸（Amended A-2 签名逐字）。
pub trait CheckGate: Send + Sync {
    /// 入站消息/媒体准入；Err = 拒（实现方自带 reason 码与审计）。
    /// media=false 判 chat.send；media=true 判 chat.attachment。
    fn admit(&self, peer: &PeerId, media: bool) -> Result<(), Reject>;
}

/// 共享闸句柄：apps 装配处注入（None = chat authz 未接线）。
pub type Gate = Option<Arc<dyn CheckGate>>;

/// 入站整帧判定（A-1 判定次序）：先 send 闸；携带媒体再过 attachment 闸；
/// 任一 Deny 短路（attachment 闸不再调用）；无闸直接放行（现状行为）。
pub fn admit_message(gate: &Gate, peer: &PeerId, has_media: bool) -> Result<(), Reject> {
    let Some(gate) = gate else {
        return Ok(());
    };
    gate.admit(peer, false)?;
    if has_media {
        gate.admit(peer, true)?;
    }
    Ok(())
}
