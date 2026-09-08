//! 泛型签名信封（纯加法模块）：canonical JSON + Ed25519 + 时间窗，离线可验。
//!
//! 供 llm-share-offer 的 SignedOffer 与 crates/a2a 的 SignedCard 共用原语
//!（docs/design/a2a-over-p2p-design.md §4.2）。b58 serde 助手经 serde(with)
//! 属性复用；签名前像 = canonical(payload) + issued_at 小端 8 字节（时刻入签
//! 防旧信封重放，对齐 rendezvous 注册 H1 纪律）。

use serde::{Deserialize, Serialize};

use crate::{Keypair, PeerId};

/// pubkey/sig 入 JSON 用 base58 字符串（对齐 PeerId 展示），而非整数数组。
pub mod b58 {
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

/// 信封验证错误：身份绑定 / 签名 / 时间窗三类，失败路径全部显式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    #[error("payload identity does not match signing pubkey")]
    IdentityMismatch,
    #[error("signature invalid or payload tampered")]
    BadSignature,
    #[error("not yet valid: issued_at in the future")]
    NotYetValid,
    #[error("envelope expired at unix {0}")]
    Expired(u64),
    #[error("canonical json encoding failed")]
    Encoding,
}

/// 签名前像：canonical(payload) 后拼 issued_at 小端 8 字节。
pub fn payload_bytes<T: Serialize>(
    payload: &T,
    issued_at: u64,
) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(payload)?;
    bytes.extend_from_slice(&issued_at.to_le_bytes());
    Ok(bytes)
}

/// 时间窗校验：now ∈ [issued_at, issued_at + ttl_secs)。通过则返回过期时刻。
pub fn check_time(issued_at: u64, ttl_secs: u64, now: u64) -> Result<u64, VerifyError> {
    if now < issued_at {
        return Err(VerifyError::NotYetValid);
    }
    let expires = issued_at.saturating_add(ttl_secs);
    if now >= expires {
        return Err(VerifyError::Expired(expires));
    }
    Ok(expires)
}

/// 签名校验：payload canonical + issued_at 与 pubkey 的签名一致。
pub fn check_signature(
    pubkey: &[u8; 32],
    payload: &impl Serialize,
    issued_at: u64,
    sig: &[u8; 64],
) -> Result<(), VerifyError> {
    let bytes = payload_bytes(payload, issued_at).map_err(|_| VerifyError::Encoding)?;
    if !Keypair::verify(pubkey, &bytes, sig) {
        return Err(VerifyError::BadSignature);
    }
    Ok(())
}

/// 签名者身份校验：pubkey 推导的 PeerId 与载荷声明的 PeerId 一致。
pub fn check_identity(pubkey: &[u8; 32], declared: &PeerId) -> Result<(), VerifyError> {
    if &PeerId::from_public_key(pubkey) != declared {
        return Err(VerifyError::IdentityMismatch);
    }
    Ok(())
}

/// 泛型签名信封：签名覆盖载荷本体与签发时刻，pubkey 经身份绑定由使用方校验。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signed<T> {
    pub payload: T,
    /// 签发时刻（unix 秒），入签。
    pub issued_at: u64,
    #[serde(with = "b58")]
    pub pubkey: [u8; 32],
    #[serde(with = "b58")]
    pub sig: [u8; 64],
}

impl<T: Serialize> Signed<T> {
    /// 签发：payload 的合法性由使用方先自行校验（sign 不代答业务规则）。
    pub fn sign(payload: T, kp: &Keypair, issued_at: u64) -> Result<Self, VerifyError> {
        let bytes = payload_bytes(&payload, issued_at).map_err(|_| VerifyError::Encoding)?;
        Ok(Self {
            payload,
            issued_at,
            pubkey: kp.public(),
            sig: kp.sign(&bytes),
        })
    }

    /// 签名 + 时间窗校验（不含身份绑定，身份由使用方的载荷字段决定）。
    pub fn verify(&self, ttl_secs: u64, now: u64) -> Result<(), VerifyError> {
        check_signature(&self.pubkey, &self.payload, self.issued_at, &self.sig)?;
        check_time(self.issued_at, ttl_secs, now).map(|_| ())
    }
}
