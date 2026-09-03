//! 结构化错误码（§4 错误透传：结构化拒绝，不产生流水）。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("receipt malformed: {0}")]
    Malformed(String),
    #[error("receipt signature invalid: req_id={0}")]
    BadSignature(String),
    /// (limit, requested)：净差 + 在途冻结 + 本次估算超上限（§7.1，MVP A2）。
    #[error("net diff limit exceeded: limit={0}, requested={1}")]
    NetDiffExceeded(u64, u64),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("hold insufficient: est={0}, usage={1}")]
    HoldInsufficient(u64, u64),
    #[error("dispute window blocks: {0}")]
    DisputeWindow(String),
}

pub type Result<T> = std::result::Result<T, Error>;
