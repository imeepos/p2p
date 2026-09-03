//! 连接关闭原因归档（E8 调研建议 4）：断链不只是「断了」，还要可归因。
//!
//! 事件走 LifecycleEvent::ConnectionClosed（加法变体，等价事件机制）：
//! NodeEvent 冻结流被 GUI types/node_event.rs 无通配符穷举消费（只读 crate），
//! 加变体即破坏其编译；E6 已确立「独立通道加法」先例，本轮沿用。

/// 关闭原因四档。前三档为调研建议 4 要求的最低集合，Local 为本端主动
/// 关闭（挂断/关停）的诚实归档——没有它，只有故障才有原因，正常关闭
/// 会被误读成 Error。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    /// 连接池空闲回收：空闲超过阈值且无在途业务流。
    Idle,
    /// 传输层异常断链：对端消失、网络故障、协议栈错误。
    Error,
    /// 门禁拒绝断链：出站拨号或入站连接被 allowlist/denylist 拒收。
    Refused,
    /// 本端主动关闭：用户挂断或节点关停。
    Local,
}

impl CloseReason {
    pub fn as_str(self) -> &'static str {
        match self {
            CloseReason::Idle => "idle",
            CloseReason::Error => "error",
            CloseReason::Refused => "refused",
            CloseReason::Local => "local",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_labels_cover_all_variants() {
        assert_eq!(CloseReason::Idle.as_str(), "idle");
        assert_eq!(CloseReason::Error.as_str(), "error");
        assert_eq!(CloseReason::Refused.as_str(), "refused");
        assert_eq!(CloseReason::Local.as_str(), "local");
    }
}
