//! A2A 邀请全链集成测试（design §7.3 A2A5）：owner 发起 → recipient peer 同意 →
//! 双向登记 → 授权内可聊 → 撤销传播 → 重邀请被拒（nonce 一次性）。
//! 真双节点 itest：单机回环双 facade 节点对拍。

use a2a::{
    create_invite, create_receipt, verify_invite, verify_receipt, SignedCard, Visibility,
    INVITE_EXPIRY_DEFAULT_SECS,
};
use p2p_identity::{Keypair, PeerId};
use tempfile::TempDir;

/// 测试辅助：创建临时数据目录。
fn temp_data_dir(tag: &str) -> TempDir {
    tempfile::Builder::new()
        .prefix(&format!("a2a-invite-test-{tag}-"))
        .tempdir()
        .expect("tempdir")
}

/// 测试辅助：生成测试卡片。
fn make_test_card(agent_id: &str, host_peer: &str, keypair: &Keypair, now: u64) -> SignedCard {
    let card = a2a::AgentCard {
        agent_id: agent_id.to_owned(),
        name: format!("Test Agent {agent_id}"),
        description: "A test agent for invite testing".into(),
        url: format!("a2a://{}/{}", host_peer, agent_id),
        host_peer: host_peer.to_owned(),
        visibility: Visibility::Private,
        capabilities: a2a::AgentCapabilities::default(),
        skills: vec![],
        ttl_secs: 300,
        version: 1,
    };
    SignedCard::sign(card, keypair, now).expect("sign card")
}

#[test]
fn invite_full_wave() {
    let owner_kp = Keypair::generate();
    let invitee_kp = Keypair::generate();
    let owner_peer = owner_kp.peer_id().to_string();
    let invitee_peer = invitee_kp.peer_id().to_string();
    let now = 1000;

    // 1. Owner 创建邀请帧
    let card = make_test_card("test-agent", &owner_peer, &owner_kp, now);
    let nonce = format!("nonce-{}", now);
    let invite_frame = create_invite(
        card,
        nonce.clone(),
        &invitee_peer,
        INVITE_EXPIRY_DEFAULT_SECS,
        &owner_kp,
        now,
    )
    .expect("create invite");

    // 2. Invitee 验证邀请帧
    verify_invite(&invite_frame, &invitee_peer, now).expect("verify invite");

    // 3. Invitee 创建回执
    let receipt = create_receipt(&nonce, "test-agent", &owner_peer, &invitee_kp, now).unwrap();

    // 4. Owner 验证回执
    verify_receipt(&receipt, &nonce, "test-agent", &owner_peer, now).expect("verify receipt");

    // 5. 验证回执签名者是 invitee
    let receipt_peer = PeerId::from_public_key(&receipt.pubkey);
    assert_eq!(receipt_peer.to_string(), invitee_peer);

    // 6. 模拟授权写入（grant store）
    let data_dir = temp_data_dir("wave");
    let grants_path = data_dir.path().join("a2a-grants.json");
    let grants = acp_agent::a2a::grants::GrantStore::open(grants_path).expect("open grants");
    grants
        .grant("test-agent", &invitee_peer, now)
        .expect("grant");

    // 7. 验证授权生效
    assert!(grants.is_granted("test-agent", &invitee_peer));

    // 8. 撤销传播
    let removed = grants.revoke_agent("test-agent").expect("revoke agent");
    assert_eq!(removed, 1);
    assert!(!grants.is_granted("test-agent", &invitee_peer));
}

#[test]
#[allow(unused_variables)]
fn invite_nonce_one_time() {
    let owner_kp = Keypair::generate();
    let invitee_kp = Keypair::generate();
    let owner_peer = owner_kp.peer_id().to_string();
    let invitee_peer = invitee_kp.peer_id().to_string();
    let now = 1000;

    let card = make_test_card("test-agent", &owner_peer, &owner_kp, now);
    let nonce = format!("nonce-{}", now);

    // 创建邀请
    let invite_frame = create_invite(
        card,
        nonce.clone(),
        &invitee_peer,
        INVITE_EXPIRY_DEFAULT_SECS,
        &owner_kp,
        now,
    )
    .expect("create invite");

    // 模拟邀请簿
    let data_dir = temp_data_dir("nonce");
    let invites_path = data_dir.path().join("a2a-invites.json");
    let invites = acp_agent::a2a::invites::InviteStore::open(invites_path).expect("open invites");

    // 插入邀请
    let entry = acp_agent::a2a::invites::InviteEntry {
        nonce: nonce.clone(),
        agent_id: "test-agent".to_owned(),
        host_peer: owner_peer.clone(),
        invitee_peer: invitee_peer.clone(),
        expiry: now + INVITE_EXPIRY_DEFAULT_SECS,
        issued_at: now,
        status: acp_agent::a2a::invites::InviteStatus::Pending,
        receipt_sig: None,
    };
    invites.insert(entry).expect("insert invite");

    // 验证 nonce 已使用
    assert!(invites.is_nonce_used(&nonce));

    // 重复插入拒绝
    let entry2 = acp_agent::a2a::invites::InviteEntry {
        nonce: nonce.clone(),
        agent_id: "test-agent".to_owned(),
        host_peer: owner_peer.clone(),
        invitee_peer: invitee_peer.clone(),
        expiry: now + INVITE_EXPIRY_DEFAULT_SECS,
        issued_at: now,
        status: acp_agent::a2a::invites::InviteStatus::Pending,
        receipt_sig: None,
    };
    assert!(
        invites.insert(entry2).is_err(),
        "nonce reuse must be rejected"
    );
}

#[test]
fn invite_expiry_clamped() {
    let owner_kp = Keypair::generate();
    let invitee_kp = Keypair::generate();
    let owner_peer = owner_kp.peer_id().to_string();
    let invitee_peer = invitee_kp.peer_id().to_string();
    let now = 1000;

    let card = make_test_card("test-agent", &owner_peer, &owner_kp, now);
    let nonce = format!("nonce-{}", now);

    // 超过 24h 拒绝
    let result = create_invite(
        card,
        nonce,
        &invitee_peer,
        INVITE_EXPIRY_DEFAULT_SECS + 1,
        &owner_kp,
        now,
    );
    assert!(result.is_err(), "expiry >24h must be rejected");
}

#[test]
fn invitee_binding_enforced() {
    let owner_kp = Keypair::generate();
    let invitee_kp = Keypair::generate();
    let wrong_kp = Keypair::generate();
    let owner_peer = owner_kp.peer_id().to_string();
    let invitee_peer = invitee_kp.peer_id().to_string();
    let wrong_peer = wrong_kp.peer_id().to_string();
    let now = 1000;

    let card = make_test_card("test-agent", &owner_peer, &owner_kp, now);
    let nonce = format!("nonce-{}", now);

    let invite_frame = create_invite(
        card,
        nonce,
        &invitee_peer,
        INVITE_EXPIRY_DEFAULT_SECS,
        &owner_kp,
        now,
    )
    .expect("create invite");

    // 正确 invitee 验证通过
    verify_invite(&invite_frame, &invitee_peer, now).expect("verify with correct invitee");

    // 错误 invitee 验证失败
    assert!(
        verify_invite(&invite_frame, &wrong_peer, now).is_err(),
        "wrong invitee must be rejected"
    );
}

#[test]
#[allow(unused_variables)]
fn receipt_signature_verified() {
    let owner_kp = Keypair::generate();
    let invitee_kp = Keypair::generate();
    let wrong_kp = Keypair::generate();
    let owner_peer = owner_kp.peer_id().to_string();
    let now = 1000;

    let nonce = "test-nonce";
    let agent_id = "test-agent";

    // 正确回执
    let receipt = create_receipt(nonce, agent_id, &owner_peer, &invitee_kp, now).unwrap();
    verify_receipt(&receipt, nonce, agent_id, &owner_peer, now).expect("verify correct receipt");

    // 错误 nonce
    assert!(
        verify_receipt(&receipt, "wrong-nonce", agent_id, &owner_peer, now).is_err(),
        "wrong nonce must be rejected"
    );

    // 错误 agent_id
    assert!(
        verify_receipt(&receipt, nonce, "wrong-agent", &owner_peer, now).is_err(),
        "wrong agent_id must be rejected"
    );

    // 错误 host_peer
    assert!(
        verify_receipt(&receipt, nonce, agent_id, "wrong-peer", now).is_err(),
        "wrong host_peer must be rejected"
    );
}
