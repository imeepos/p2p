//! 结构化错误码（§4 错误透传）：A 侧拒绝必须机器可区分，拒绝路径上游零调用、流水零产生。

use serde::{Deserialize, Serialize};

/// 三闸拒绝与失败路径各占一码，wire 层直接承载。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// 借方 PeerId 不在出借方 allowlist（闸 1，MVP A5）。
    NotAllowlisted,
    /// 请求模型不在出借方白名单（闸 2）。
    ModelNotServed,
    /// 并发超限（§7.1 四件套）。
    ConcurrencyExceeded,
    /// 预授权冻结不足/净差超限（闸 3，MVP A2）。
    FreezeInsufficient,
    /// req_id 已结算或在途，重放去重（MVP A4）。
    DuplicateReqId,
    /// 请求帧非法（缺 req_id/model/max_tokens）。
    BadRequest,
    /// 上游 4xx/5xx，状态码透传（§4 错误透传，不产生流水）。
    UpstreamRejected,
    /// 上游流中途断开且未给出 usage（断流计费入口，MVP A6）。
    UpstreamStreamBroken,
    /// 结算失败（实际 usage 超冻结额等），冻结保留待人工处置。
    SettleFailed,
    /// 出借方内部错误。
    Internal,
}

/// 客户端侧错误：传输与收据验签失败显式上抛，不静默。
#[derive(Debug, thiserror::Error)]
pub enum ProxyClientError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("timeout after {0:?}")]
    Timeout(std::time::Duration),
    #[error("wire: {0}")]
    Wire(String),
    #[error("receipt signature invalid: req_id={0}")]
    BadReceipt(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_snake_case_roundtrip() {
        for code in [
            ErrorCode::NotAllowlisted,
            ErrorCode::ModelNotServed,
            ErrorCode::ConcurrencyExceeded,
            ErrorCode::FreezeInsufficient,
            ErrorCode::DuplicateReqId,
            ErrorCode::UpstreamStreamBroken,
        ] {
            let json = serde_json::to_string(&code).expect("serializable");
            assert_eq!(code, serde_json::from_str(&json).expect("roundtrip"));
        }
        assert_eq!(
            serde_json::to_string(&ErrorCode::NotAllowlisted).expect("serializable"),
            "\"not_allowlisted\""
        );
    }
}
