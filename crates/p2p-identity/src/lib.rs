//! 节点身份：Ed25519 密钥对与 PeerId。
//!
//! PeerId = base58(sha256(ed25519 公钥原始 32 字节))，身份与密钥绑定，
//! 握手阶段即可完成互认（见 docs/design/p2p-base-design.md §6）。
//! 种子落盘持久化由内核会话（kernel-transport）补充。

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::fmt;

/// 节点标识，32 字节定长。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerId([u8; 32]);

impl PeerId {
    pub fn from_public_key(pubkey: &[u8; 32]) -> Self {
        Self(Sha256::digest(pubkey).into())
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&bs58::encode(self.0).into_string())
    }
}

impl fmt::Debug for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PeerId({self})")
    }
}

/// Ed25519 密钥对，节点长期身份。
#[derive(Clone)]
pub struct Keypair(SigningKey);

impl Keypair {
    /// 用操作系统 CSPRNG 生成新身份。
    pub fn generate() -> Self {
        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        Self(SigningKey::from_bytes(&seed))
    }

    /// 从 32 字节种子恢复。
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        Self(SigningKey::from_bytes(seed))
    }

    pub fn to_seed_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    pub fn public(&self) -> [u8; 32] {
        self.0.verifying_key().to_bytes()
    }

    pub fn peer_id(&self) -> PeerId {
        PeerId::from_public_key(&self.public())
    }

    pub fn sign(&self, msg: &[u8]) -> [u8; 64] {
        self.0.sign(msg).to_bytes()
    }

    /// 独立验证：公钥 + 消息 + 签名，供 rendezvous 注册等场景使用。
    pub fn verify(pubkey: &[u8; 32], msg: &[u8], sig: &[u8; 64]) -> bool {
        let Ok(vk) = VerifyingKey::from_bytes(pubkey) else {
            return false;
        };
        vk.verify(msg, &Signature::from_bytes(sig)).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_id_roundtrip_and_sign() {
        let kp = Keypair::generate();
        let peer = kp.peer_id();
        assert_eq!(PeerId::from_bytes(*peer.as_bytes()), peer);
        let sig = kp.sign(b"hello");
        assert!(Keypair::verify(&kp.public(), b"hello", &sig));
        assert!(!Keypair::verify(&kp.public(), b"tampered", &sig));
    }

    #[test]
    fn seed_restore() {
        let kp = Keypair::generate();
        let restored = Keypair::from_seed(&kp.to_seed_bytes());
        assert_eq!(kp.public(), restored.public());
    }
}
