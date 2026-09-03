//! 能力声明模型（idle-token-sharing-plan §5.2）与签名信封：canonical JSON + Ed25519，离线可验。
//!
//! retention 为出借方数据留存自述（§7.3，协调者裁决必含，如 "none"），
//! 本 crate 只透传不解释，语义由选路使用方裁量。

use std::collections::BTreeMap;

use p2p_identity::{Keypair, PeerId};
use serde::{Deserialize, Serialize};

/// 协议 ID（wire-protocol §3：/命名空间/名字/版本，JSON 编码）。
pub const PROTOCOL_ID: &str = "/llm-share/offer/1";

/// 能力声明：出借方闲量自述，字段即 §5.2 wire 格式。
/// spare/max_per_req 用 BTreeMap 保证 canonical 字节键序稳定，签名可复现。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    /// 出借方 PeerId（base58）。
    pub peer: String,
    pub models: Vec<String>,
    /// 模型 -> 声明闲量 token（软约束，预授权实校为准）。
    pub spare: BTreeMap<String, u64>,
    /// 账期截止日（如 "2026-09-30"）。
    pub period_ends: String,
    /// 模型 -> 单请求 max_tokens 上限；缺条目 = 未显式设限（预授权仍实校）。
    pub max_per_req: BTreeMap<String, u64>,
    pub rate_limit: RateLimit,
    /// 声明有效期（秒），自 issued_at 起算，过期即失效（A5）。
    #[serde(rename = "ttl")]
    pub ttl_secs: u64,
    /// 数据留存自述（如 "none"），如实告知供 B 选路参考（§7.3）。
    pub retention: String,
}

/// 速率限额两项（§7.1 四件套的声明内子集；净差上限在账本侧，不入声明）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimit {
    pub rpm: u32,
    pub concurrency: u32,
}

/// 声明校验错误。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OfferError {
    #[error("{0} must not be empty")]
    Empty(&'static str),
    #[error("spare for model {0} missing or zero")]
    SparePositive(String),
    #[error("map key references unknown model {0}")]
    UnknownModel(String),
    #[error("{0} must be positive")]
    Positive(&'static str),
    #[error("canonical json encoding failed: {0}")]
    Encoding(String),
}

impl Offer {
    /// 声明合法性：非空字段、models 与 spare 互相覆盖且闲量 > 0（u64 类型层面排除负数，
    /// 声明零闲量等于没这能力，直接拒）、关联键不越出 models、限速与 TTL 为正。
    pub fn validate(&self) -> Result<(), OfferError> {
        if self.peer.is_empty() {
            return Err(OfferError::Empty("peer"));
        }
        if self.period_ends.is_empty() {
            return Err(OfferError::Empty("period_ends"));
        }
        if self.retention.is_empty() {
            return Err(OfferError::Empty("retention"));
        }
        if self.models.is_empty() {
            return Err(OfferError::Empty("models"));
        }
        for model in &self.models {
            if !self.spare.get(model).is_some_and(|v| *v > 0) {
                return Err(OfferError::SparePositive(model.clone()));
            }
        }
        self.check_model_keys(self.spare.keys())?;
        self.check_model_keys(self.max_per_req.keys())?;
        if self.max_per_req.values().any(|v| *v == 0) {
            return Err(OfferError::Positive("max_per_req"));
        }
        if self.rate_limit.rpm == 0 || self.rate_limit.concurrency == 0 {
            return Err(OfferError::Positive("rate_limit"));
        }
        if self.ttl_secs == 0 {
            return Err(OfferError::Positive("ttl"));
        }
        Ok(())
    }

    fn check_model_keys<'a, I: Iterator<Item = &'a String>>(
        &self,
        keys: I,
    ) -> Result<(), OfferError> {
        for key in keys {
            if !self.models.contains(key) {
                return Err(OfferError::UnknownModel(key.clone()));
            }
        }
        Ok(())
    }

    /// canonical 字节：serde_json 紧凑序列化（BTreeMap 键序确定），作为签名基准。
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, OfferError> {
        serde_json::to_vec(self).map_err(|e| OfferError::Encoding(e.to_string()))
    }
}

/// pubkey/sig 入 JSON 用 base58 字符串（对齐 PeerId 展示），而非整数数组。
mod b58 {
    use serde::{de::Error, ser::Serializer, Deserialize, Deserializer};

    pub fn serialize<S, const N: usize>(v: &[u8; N], s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(&bs58::encode(v).into_string())
    }

    pub fn deserialize<'de, D, const N: usize>(d: D) -> Result<[u8; N], D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = bs58::decode(String::deserialize(d)?)
            .into_vec()
            .map_err(D::Error::custom)?;
        raw.try_into()
            .map_err(|_| D::Error::custom("base58 payload length mismatch"))
    }
}

/// 签名前像：canonical(offer) 后拼 issued_at 小端 8 字节，时刻入签防旧声明重放
///（对齐 rendezvous 注册的 H1 纪律）。
fn signing_payload(offer: &Offer, issued_at: u64) -> Result<Vec<u8>, OfferError> {
    let mut payload = offer.canonical_bytes()?;
    payload.extend_from_slice(&issued_at.to_le_bytes());
    Ok(payload)
}

/// 签名声明信封：签名覆盖声明本体与签发时刻，pubkey 绑定声明内 peer。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedOffer {
    pub offer: Offer,
    /// 签发时刻（unix 秒），入签。
    pub issued_at: u64,
    #[serde(with = "b58")]
    pub pubkey: [u8; 32],
    #[serde(with = "b58")]
    pub sig: [u8; 64],
}

/// 信封验证错误：peer 绑定 / 签名 / 时间窗三类，失败路径全部显式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    #[error("offer.peer does not match signing pubkey")]
    PeerMismatch,
    #[error("signature invalid or payload tampered")]
    BadSignature,
    #[error("not yet valid: issued_at in the future")]
    NotYetValid,
    #[error("offer expired at unix {0}")]
    Expired(u64),
    #[error("canonical json encoding failed")]
    Encoding,
}

impl SignedOffer {
    /// 签发：先校验声明合法性，拒签非法声明。
    pub fn sign(offer: &Offer, kp: &Keypair, issued_at: u64) -> Result<Self, OfferError> {
        offer.validate()?;
        let payload = signing_payload(offer, issued_at)?;
        Ok(Self {
            offer: offer.clone(),
            issued_at,
            pubkey: kp.public(),
            sig: kp.sign(&payload),
        })
    }

    /// 声明 peer 的 PeerId；base58 解析失败即信封损坏，返回 None。
    pub fn peer_id(&self) -> Option<PeerId> {
        let raw: [u8; 32] = bs58::decode(&self.offer.peer)
            .into_vec()
            .ok()?
            .try_into()
            .ok()?;
        Some(PeerId::from_bytes(raw))
    }

    /// 过期时刻（unix 秒）：issued_at + ttl。
    pub fn expires_at(&self) -> u64 {
        self.issued_at.saturating_add(self.offer.ttl_secs)
    }

    /// 离线验证：peer 与 pubkey 绑定、签名、时间窗（now ∈ [issued_at, issued_at+ttl)）。
    /// TTL 过期即失效（A5 后半），任何一项不过都拒。
    pub fn verify(&self, now: u64) -> Result<(), VerifyError> {
        let Some(peer) = self.peer_id() else {
            return Err(VerifyError::PeerMismatch);
        };
        if peer != PeerId::from_public_key(&self.pubkey) {
            return Err(VerifyError::PeerMismatch);
        }
        let payload =
            signing_payload(&self.offer, self.issued_at).map_err(|_| VerifyError::Encoding)?;
        if !Keypair::verify(&self.pubkey, &payload, &self.sig) {
            return Err(VerifyError::BadSignature);
        }
        if now < self.issued_at {
            return Err(VerifyError::NotYetValid);
        }
        if now >= self.expires_at() {
            return Err(VerifyError::Expired(self.expires_at()));
        }
        Ok(())
    }
}
