//! 桥错误码：wire 面（denied 帧只带 code() 词法码）与审计面（Display，含上限值，
//! 不含敏感负载）分离。变体载荷只有数值上限，结构上杜绝敏感值入日志。

/// 桥全线错误码。code() 供握手 denied 帧与日志检索键；Display 面向审计日志。
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ErrorCode {
    #[error("peer-not-allowed: transport peer has no policy grant")]
    PeerNotAllowed,
    #[error("line-too-long: ndjson line exceeded guard limit ({limit_bytes} bytes)")]
    LineTooLong { limit_bytes: usize },
    #[error(
        "frame-too-large: frame of {size_bytes} bytes exceeds chunk limit ({limit_bytes} bytes)"
    )]
    FrameTooLarge {
        size_bytes: usize,
        limit_bytes: usize,
    },
    #[error("subprocess-failed: bridge subprocess spawn or attach failed")]
    SubprocessFailed,
    #[error("session-cap-reached: per-connection session cap ({cap}) exceeded")]
    SessionCapReached { cap: u32 },
    #[error("conn-cap-reached: per-peer concurrent connection cap ({cap}) exceeded")]
    ConnCapReached { cap: u32 },
    #[error("reattach-ticket-invalid: reattach ticket unknown, expired, or cross-device")]
    ReattachTicketInvalid,
    #[error("cwd-denied: subprocess working directory outside policy jail")]
    CwdDenied,
    #[error("handshake-malformed: handshake frame failed validation")]
    HandshakeMalformed,
    #[error("ndjson-truncated: stream ended with an unterminated line")]
    NdjsonTruncated,
}

impl ErrorCode {
    /// wire 词法码（denied 帧 payload），与 Display 前缀一致。
    pub fn code(&self) -> &'static str {
        match self {
            Self::PeerNotAllowed => "peer-not-allowed",
            Self::LineTooLong { .. } => "line-too-long",
            Self::FrameTooLarge { .. } => "frame-too-large",
            Self::SubprocessFailed => "subprocess-failed",
            Self::SessionCapReached { .. } => "session-cap-reached",
            Self::ConnCapReached { .. } => "conn-cap-reached",
            Self::ReattachTicketInvalid => "reattach-ticket-invalid",
            Self::CwdDenied => "cwd-denied",
            Self::HandshakeMalformed => "handshake-malformed",
            Self::NdjsonTruncated => "ndjson-truncated",
        }
    }
}
