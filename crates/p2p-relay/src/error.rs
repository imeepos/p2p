//! 客户端/服务端共享错误：本地类型化错误 + 服务端拒绝帧映射。

use crate::messages::errcode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RelayError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// 服务端 Reject 帧（携带错误码与可读信息）。
    #[error("relay rejected: code={code}, {message}")]
    Server { code: u32, message: String },
    #[error("peer limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("protocol violation: {0}")]
    Protocol(String),
    #[error("control link closed")]
    LinkClosed,
    #[error("timeout waiting for {0}")]
    Timeout(&'static str),
}

impl RelayError {
    /// 本地错误转 wire (code, message)，服务端写 Reject 帧用。
    pub fn to_wire(&self) -> (u32, String) {
        match self {
            RelayError::Server { code, message } => (*code, message.clone()),
            RelayError::LimitExceeded(m) => (errcode::PEER_LIMIT, m.clone()),
            other => (errcode::PROTOCOL, other.to_string()),
        }
    }
}

/// 服务端 Reject 帧转 RelayError。
pub fn error_from_wire(code: u32, message: String) -> RelayError {
    RelayError::Server { code, message }
}
