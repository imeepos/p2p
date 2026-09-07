//! AgentCard/SignedCard 单测（行数红线拆出，src-tauri 同款 *_tests.rs 约定）。
use crate::card::*;
use p2p_identity::Keypair;
use std::time::{SystemTime, UNIX_EPOCH};
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 校验类测试用卡：host 内部生成，不做身份绑定断言。
fn sample_card(agent_id: &str) -> AgentCard {
    let host = Keypair::generate().peer_id().to_string();
    sample_card_with_host(&host, agent_id)
}

fn sample_card_with_host(host: &str, agent_id: &str) -> AgentCard {
    AgentCard {
        agent_id: agent_id.to_string(),
        name: "代码评审员".into(),
        description: "对 PR 做代码评审".into(),
        url: format!("a2a://{host}/{agent_id}"),
        host_peer: host.to_string(),
        visibility: Visibility::Public,
        capabilities: AgentCapabilities { streaming: true },
        skills: vec![AgentSkill {
            id: "code-review".into(),
            name: "代码评审".into(),
            description: None,
            tags: vec!["review".into()],
        }],
        ttl_secs: TTL_DEFAULT_SECS,
        version: 1,
    }
}

#[test]
fn validate_rejects_bad_fields() {
    let mut card = sample_card("good-id");
    assert!(card.validate().is_ok());
    card.agent_id = "Bad_ID".into();
    assert!(card.validate().is_err());
    card.agent_id = "a".repeat(40);
    assert!(card.validate().is_err());
    card.agent_id = "代码".into();
    assert!(card.validate().is_err());
    card.agent_id = "good-id".into();
    card.ttl_secs = 0;
    assert!(card.validate().is_err());
    card.ttl_secs = TTL_MAX_SECS + 1;
    assert!(card.validate().is_err());
    card.ttl_secs = TTL_DEFAULT_SECS;
    card.url = "http://evil".into();
    assert!(card.validate().is_err());
    card.skills = (0..=SKILLS_MAX)
        .map(|i| AgentSkill {
            id: format!("s{i}"),
            name: format!("技能{i}"),
            description: None,
            tags: vec![],
        })
        .collect();
    assert!(card.validate().is_err());
}

#[test]
fn sign_verify_roundtrip_and_tamper() {
    let kp = Keypair::generate();
    let t = now();
    let signed = SignedCard::sign(
        sample_card_with_host(&kp.peer_id().to_string(), "roundtrip"),
        &kp,
        t,
    )
    .expect("sign ok");
    assert_eq!(signed.host_peer_id(), Some(kp.peer_id()));
    assert!(signed.verify(t).is_ok());
    assert!(signed.verify(t + 1).is_ok());
    assert!(
        signed.verify(t + TTL_DEFAULT_SECS).is_err(),
        "TTL 到期必须失败"
    );
    assert!(
        signed.verify(t.saturating_sub(10)).is_err(),
        "未来签发必须失败"
    );
}

#[test]
fn sign_verify_rejects_wrong_host() {
    // host_peer 与签名者不一致 → 绑定失败（防冒名）
    let kp = Keypair::generate();
    let other = Keypair::generate();
    let mut card = sample_card("spoof");
    card.host_peer = other.peer_id().to_string();
    card.url = format!("a2a://{}/spoof", other.peer_id());
    let signed = SignedCard::sign(card, &kp, now()).expect("sign ok");
    assert!(
        signed.verify(now()).is_err(),
        "pubkey 与 hostPeer 绑定必须拦下冒名"
    );
}

#[test]
fn sign_rejects_invalid_card() {
    let kp = Keypair::generate();
    let mut card = sample_card("bad");
    card.ttl_secs = 0;
    assert!(
        SignedCard::sign(card, &kp, now()).is_err(),
        "非法声明拒绝签发"
    );
}
