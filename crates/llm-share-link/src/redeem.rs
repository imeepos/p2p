//! 兑换协议帧（/llm-share/redeem/1）：借方发 {token}，出借方回 {ok:true} 或
//! 结构化拒绝码（share-revoked/expired/exhausted/bound-other/invalid）；零流水。
//! 失败响应包络统一，不外泄 share 是否存在（设计 §5.4 C 段 + 契约 §16.6）。

use serde::{Deserialize, Serialize};

use crate::ledger::RedeemError;

/// 协议 ID（wire-protocol §3：/命名空间/名字/版本，JSON 编码）。
pub const PROTOCOL_ID: &str = "/llm-share/redeem/1";

/// 兑换请求帧。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedeemRequest {
    pub token: String,
}

/// 结构化拒绝码（wire kebab-case）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RedeemCode {
    ShareRevoked,
    Expired,
    Exhausted,
    BoundOther,
    Invalid,
}

/// 兑换响应帧：成功无 code，失败带单一 code。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedeemResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<RedeemCode>,
}

impl RedeemResponse {
    /// 成功响应（{ok:true}，无 code 字段）。
    pub fn ok() -> Self {
        Self {
            ok: true,
            code: None,
        }
    }

    /// 业务拒绝响应（{ok:false, code:...}）。
    pub fn deny(code: RedeemCode) -> Self {
        Self {
            ok: false,
            code: Some(code),
        }
    }
}

impl From<RedeemError> for RedeemCode {
    fn from(e: RedeemError) -> Self {
        match e {
            RedeemError::Invalid => RedeemCode::Invalid,
            RedeemError::Revoked => RedeemCode::ShareRevoked,
            RedeemError::Expired => RedeemCode::Expired,
            RedeemError::Exhausted => RedeemCode::Exhausted,
            RedeemError::BoundOther => RedeemCode::BoundOther,
        }
    }
}

#[cfg(test)]
mod tests;
