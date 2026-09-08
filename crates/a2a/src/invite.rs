//! 邀请帧模型（docs/design/a2a-over-p2p-design.md §7.3）：签名凭证邀请帧，
//! 对齐 social-discovery-plan §P2 + im/invite/1 纪律。wire 形态：
//! SignedCard + nonce（一次性，宿主持久化防重放）+ expiry（默认 24h）+
//! invitee PeerId 绑定 + owner 签名。回执 = invitee 签名(nonce, agentId, hostPeer)。

use p2p_identity::{signed::Signed, Keypair, PeerId};
use serde::{Deserialize, Serialize};

use crate::card::SignedCard;

/// 邀请帧最大有效期（秒）：24h，设计拍板。
pub const INVITE_EXPIRY_MAX_SECS: u64 = 24 * 60 * 60;
/// 邀请帧默认有效期（秒）：24h。
pub const INVITE_EXPIRY_DEFAULT_SECS: u64 = INVITE_EXPIRY_MAX_SECS;

/// 邀请帧 payload（owner 签名前的声明本体）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InvitePayload {
    /// 被邀请的 agent 卡片（SignedCard 信封）。
    pub card: SignedCard,
    /// 一次性 nonce（UUID v4 简化版，宿主持久化防重放）。
    pub nonce: String,
    /// 邀请有效期截止（unix secs）：= issued_at + expiry，钳制 ≤24h。
    pub expiry: u64,
    /// 被邀请方 PeerId（base58），绑定防放大授权。
    pub invitee_peer: String,
}

/// 邀请帧信封：Signed<InvitePayload> + owner 签名。
pub type InviteFrame = Signed<InvitePayload>;

/// 邀请帧校验错误。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InviteError {
    #[error("nonce 为空")]
    EmptyNonce,
    #[error("expiry 超过 24h 上限（{0}s > {INVITE_EXPIRY_MAX_SECS}s）")]
    ExpiryTooLong(u64),
    #[error("invitee_peer 不是合法 base58 PeerId")]
    InviteePeerInvalid,
    #[error("签名验证失败: {0}")]
    Verify(String),
    #[error("邀请已过期")]
    Expired,
    #[error("nonce 已使用（一次性拒绝）")]
    NonceReused,
}

/// 构造邀请帧：owner 签名，expiry 钳制 ≤24h。
pub fn create_invite(
    card: SignedCard,
    nonce: String,
    invitee_peer: &str,
    expiry: u64,
    keypair: &Keypair,
    now: u64,
) -> Result<InviteFrame, InviteError> {
    if nonce.is_empty() {
        return Err(InviteError::EmptyNonce);
    }
    if expiry > INVITE_EXPIRY_MAX_SECS {
        return Err(InviteError::ExpiryTooLong(expiry));
    }
    // 校验 invitee_peer 是合法 PeerId（base58 解码）
    let raw: [u8; 32] = bs58::decode(invitee_peer)
        .into_vec()
        .map_err(|_| InviteError::InviteePeerInvalid)?
        .try_into()
        .map_err(|_| InviteError::InviteePeerInvalid)?;
    let _ = PeerId::from_bytes(raw);

    let payload = InvitePayload {
        card,
        nonce,
        expiry: now + expiry,
        invitee_peer: invitee_peer.to_owned(),
    };
    Signed::sign(payload, keypair, now).map_err(|e| InviteError::Verify(e.to_string()))
}

/// 验证邀请帧：验签 + 时间窗 + invitee 绑定。
pub fn verify_invite(
    frame: &InviteFrame,
    expected_invitee: &str,
    now: u64,
) -> Result<(), InviteError> {
    // 计算 ttl（expiry - issued_at）
    let ttl = frame.payload.expiry.saturating_sub(frame.issued_at);
    // 验签 + 时间窗
    frame
        .verify(ttl, now)
        .map_err(|e| InviteError::Verify(e.to_string()))?;
    // invitee 绑定校验
    if frame.payload.invitee_peer != expected_invitee {
        return Err(InviteError::InviteePeerInvalid);
    }
    // expiry 校验（Signed 时间窗已覆盖，此处双重确认）
    if now >= frame.payload.expiry {
        return Err(InviteError::Expired);
    }
    Ok(())
}

/// 构造回执：invitee 签名(nonce, agentId, hostPeer)。
pub fn create_receipt(
    nonce: &str,
    agent_id: &str,
    host_peer: &str,
    keypair: &Keypair,
    now: u64,
) -> Result<Signed<ReceiptPayload>, InviteError> {
    let payload = ReceiptPayload {
        nonce: nonce.to_owned(),
        agent_id: agent_id.to_owned(),
        host_peer: host_peer.to_owned(),
    };
    Signed::sign(payload, keypair, now).map_err(|e| InviteError::Verify(e.to_string()))
}

/// 回执 payload：invitee 签名绑定。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReceiptPayload {
    pub nonce: String,
    pub agent_id: String,
    pub host_peer: String,
}

/// 验证回执：验签 + nonce/agentId/hostPeer 绑定。
pub fn verify_receipt(
    receipt: &Signed<ReceiptPayload>,
    expected_nonce: &str,
    expected_agent_id: &str,
    expected_host_peer: &str,
    now: u64,
) -> Result<(), InviteError> {
    // 回执无 expiry，用大 ttl 仅验签名
    receipt
        .verify(86400, now)
        .map_err(|e| InviteError::Verify(e.to_string()))?;
    if receipt.payload.nonce != expected_nonce {
        return Err(InviteError::Verify("nonce mismatch".into()));
    }
    if receipt.payload.agent_id != expected_agent_id {
        return Err(InviteError::Verify("agent_id mismatch".into()));
    }
    if receipt.payload.host_peer != expected_host_peer {
        return Err(InviteError::Verify("host_peer mismatch".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{AgentCapabilities, AgentCard, Visibility};

    fn test_keypair() -> Keypair {
        Keypair::generate()
    }

    fn make_test_card(agent_id: &str, host_peer: &str) -> AgentCard {
        AgentCard {
            agent_id: agent_id.to_owned(),
            name: "Test Agent".into(),
            description: "A test agent".into(),
            url: format!("a2a://{}/{}", host_peer, agent_id),
            host_peer: host_peer.to_owned(),
            visibility: Visibility::Public,
            capabilities: AgentCapabilities::default(),
            skills: vec![],
            ttl_secs: 300,
            version: 1,
        }
    }

    #[test]
    fn create_and_verify_invite_roundtrip() {
        let owner_kp = test_keypair();
        let invitee_kp = test_keypair();
        let invitee_peer = invitee_kp.peer_id().to_string();
        let host_peer = owner_kp.peer_id().to_string();
        let now = 1000;

        let card = make_test_card("test-agent", &host_peer);
        let signed_card = SignedCard::sign(card, &owner_kp, now).unwrap();
        let nonce = format!("nonce-{}", now);

        let frame = create_invite(
            signed_card,
            nonce.clone(),
            &invitee_peer,
            3600,
            &owner_kp,
            now,
        )
        .unwrap();
        assert!(verify_invite(&frame, &invitee_peer, now).is_ok());
        // 过期
        assert!(verify_invite(&frame, &invitee_peer, frame.payload.expiry).is_err());
        // 错误 invitee
        assert!(verify_invite(&frame, "wrong-peer", now).is_err());
    }

    #[test]
    fn expiry_clamped_to_24h() {
        let owner_kp = test_keypair();
        let invitee_peer = test_keypair().peer_id().to_string();
        let host_peer = owner_kp.peer_id().to_string();
        let now = 1000;
        let card = make_test_card("test-agent", &host_peer);
        let signed_card = SignedCard::sign(card, &owner_kp, now).unwrap();
        let nonce = format!("nonce-{}", now);

        // 超过 24h 拒绝
        let result = create_invite(
            signed_card,
            nonce,
            &invitee_peer,
            INVITE_EXPIRY_MAX_SECS + 1,
            &owner_kp,
            now,
        );
        assert!(matches!(result, Err(InviteError::ExpiryTooLong(_))));
    }

    #[test]
    fn empty_nonce_rejected() {
        let owner_kp = test_keypair();
        let invitee_peer = test_keypair().peer_id().to_string();
        let host_peer = owner_kp.peer_id().to_string();
        let now = 1000;
        let card = make_test_card("test-agent", &host_peer);
        let signed_card = SignedCard::sign(card, &owner_kp, now).unwrap();

        let result = create_invite(signed_card, "".into(), &invitee_peer, 3600, &owner_kp, now);
        assert!(matches!(result, Err(InviteError::EmptyNonce)));
    }

    #[test]
    fn receipt_roundtrip() {
        let owner_kp = test_keypair();
        let invitee_kp = test_keypair();
        let nonce = "test-nonce";
        let agent_id = "test-agent";
        let host_peer = owner_kp.peer_id().to_string();
        let now = 1000;

        let receipt = create_receipt(nonce, agent_id, &host_peer, &invitee_kp, now).unwrap();
        assert!(verify_receipt(&receipt, nonce, agent_id, &host_peer, now).is_ok());
        // nonce 不匹配
        assert!(verify_receipt(&receipt, "wrong", agent_id, &host_peer, now).is_err());
    }
}
