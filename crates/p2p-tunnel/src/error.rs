//! 隧道错误面：结构化拒绝（错误帧映射）与 IO 故障分层（契约 §3/§4）。

use crate::wire::TunnelErrorCode;

#[derive(Debug, thiserror::Error)]
pub enum TunnelError {
    /// 被访侧显式 error 帧（错误码闭集，契约 §3）。
    #[error("tunnel rejected: {code} ({message})")]
    Rejected {
        code: TunnelErrorCode,
        message: String,
    },
    /// 传输/帧/本地 IO 故障。
    #[error("tunnel io: {0}")]
    Io(#[from] std::io::Error),
}
