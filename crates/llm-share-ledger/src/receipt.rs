//! 收据：记账唯一凭据（§5.1）。sig 为除 sig 外全字段规范化 JSON 的 Ed25519 签名。

use p2p_identity::Keypair;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
}

/// 单笔双边流水凭据；lender/borrower 为 PeerId base58 文本，period 为出借方本地账期。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub v: u8,
    pub req_id: String,
    pub period: String,
    pub lender: String,
    pub borrower: String,
    pub model: String,
    pub usage: Usage,
    pub estimated: bool,
    pub upstream_hint: String,
    pub ts: u64,
    pub sig: String,
}

impl Receipt {
    /// 规范化序列化：键排序、无空白、剔除 sig，键序重排不影响验签。
    pub fn canonical_payload(&self) -> Result<Vec<u8>> {
        let mut value = serde_json::to_value(self).map_err(|e| Error::Malformed(e.to_string()))?;
        value.as_object_mut().ok_or_else(|| Error::Malformed("not object".into()))?.remove("sig");
        serde_json::to_vec(&value).map_err(|e| Error::Malformed(e.to_string()))
    }

    /// 出借方签名，覆盖旧 sig。
    pub fn sign(&mut self, lender: &Keypair) -> Result<()> {
        self.sig.clear();
        self.sig = bs58::encode(lender.sign(&self.canonical_payload()?)).into_string();
        Ok(())
    }

    /// 出借方公钥验签；任何字段篡改都会改变规范化 payload 而失败（MVP A3）。
    pub fn verify(&self, lender_pubkey: &[u8; 32]) -> Result<()> {
        let raw = bs58::decode(&self.sig).into_vec().map_err(|_| Error::Malformed("sig".into()))?;
        let sig =
            <[u8; 64]>::try_from(raw.as_slice()).map_err(|_| Error::Malformed("sig".into()))?;
        if Keypair::verify(lender_pubkey, &self.canonical_payload()?, &sig) {
            Ok(())
        } else {
            Err(Error::BadSignature(self.req_id.clone()))
        }
    }
}
