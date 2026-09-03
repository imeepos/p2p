//! 声明发布与订阅：经 p2p-discovery 的 rendezvous 签名注册通道发布（专用 namespace，
//! 复用其 sign_register/Registry 公开设施，不改其代码）；订阅侧声明簿 TTL 过期即失效。

use std::collections::HashMap;

use p2p_discovery::rendezvous::messages::{sign_register, Register};
use p2p_identity::{Keypair, PeerId};
use p2p_transport::TransportAddr;
use tracing::warn;

use crate::offer::{SignedOffer, VerifyError};

/// 能力声明的 rendezvous namespace：签名覆盖，与会话发现租户隔离。
pub const OFFER_NAMESPACE: &str = "llm-share/offer/1";

/// 组装在场宣告注册帧：声明 TTL 即注册 TTL（服务端封顶 3600s），发布方按其周期刷新；
/// 声明本体经 SignedOffer 信封单独交换，由订阅侧验签后入簿。
pub fn announce_register(
    kp: &Keypair,
    addrs: &[TransportAddr],
    ttl_secs: u32,
    issued_at: u64,
) -> Register {
    sign_register(kp, OFFER_NAMESPACE, addrs, ttl_secs, issued_at)
}

/// 订阅侧声明簿：只收验签与时间窗通过的声明；同 peer 重复发布覆盖旧条目
///（最新声明 wins；乱序到达的旧帧若仍在有效期内会覆盖新帧，MVP 白名单场景可接受）。
#[derive(Debug, Default)]
pub struct OfferBook {
    entries: HashMap<PeerId, SignedOffer>,
}

impl OfferBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// 验签与时间窗通过才入簿；失败路径透传错误由调用方留痕，不静默吞。
    pub fn insert(&mut self, signed: SignedOffer, now: u64) -> Result<(), VerifyError> {
        let peer = signed.peer_id().ok_or(VerifyError::PeerMismatch)?;
        signed.verify(now)?;
        self.entries.insert(peer, signed);
        Ok(())
    }

    /// 当前仍有效的声明（按声明 TTL 过滤，过期即不被看到——A5 后半）。
    pub fn live(&self, now: u64) -> Vec<&SignedOffer> {
        self.entries
            .values()
            .filter(|s| now < s.expires_at())
            .collect()
    }

    /// 移除过期声明并返回 peer 清单（对应 Expired 语义），WARN 留观测信号。
    pub fn evict_expired(&mut self, now: u64) -> Vec<PeerId> {
        let expired: Vec<PeerId> = self
            .entries
            .iter()
            .filter(|(_, s)| now >= s.expires_at())
            .map(|(p, _)| *p)
            .collect();
        for peer in &expired {
            self.entries.remove(peer);
            warn!(target: "llm_share_offer", peer = %peer, "offer expired, evicted");
        }
        expired
    }

    pub fn get(&self, peer: &PeerId) -> Option<&SignedOffer> {
        self.entries.get(peer)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
