//! A2A 邀请真实链路 E2E（#[ignore] + SKIP 信号 + A2A_REAL_CHAIN 开关）。
//! 依赖 acp-agent 真实构建（admin HTTP 管道）；不可用时直写 stderr 留 SKIP 信号。

mod a2a_card_common;

use a2a::verify_invite;

/// 真链路邀请 E2E：admin HTTP 创建 agent → POST /a2a/agents/{id}/invite →
/// 验证邀请帧签名 + nonce 落盘。依赖 acp-agent 真实构建（admin HTTP 管道）；
/// 构建不可用时直写 stderr 留 SKIP 信号，不假绿。
#[tokio::test]
#[ignore] // 需显式 --ignored 触发；默认跳过，CI 环境可配 A2A_REAL_CHAIN=1
async fn t6_admin_invite_e2e() {
    use a2a_card_common::{admin_get, admin_post, host_rig};
    use p2p_identity::Keypair;

    if std::env::var("A2A_REAL_CHAIN").unwrap_or_default() != "1" {
        use std::io::Write as _;
        let _ = std::io::stderr()
            .write_all(b"SKIP: A2A_REAL_CHAIN != 1 (set A2A_REAL_CHAIN=1 or use --ignored)\n");
        return;
    }

    let host = host_rig("t6-invite").await;
    let invitee_kp = Keypair::generate();
    let invitee_peer = invitee_kp.peer_id().to_string();

    host.agents
        .create(
            Some("invite-agent".into()),
            "邀请测试".into(),
            "desc".into(),
            vec![],
            a2a::Visibility::Private,
            1,
        )
        .expect("create agent");

    eprintln!("t6: admin_addr={}", host.admin_addr);
    eprintln!("t6: token_len={}", host.admin_token.len());
    let (get_status, get_body) = admin_get(host.admin_addr, &host.admin_token, "/a2a/agents").await;
    eprintln!(
        "t6: GET /a2a/agents status={get_status} body_len={} body={}",
        get_body.len(),
        get_body
    );
    assert_eq!(get_status, 200, "admin GET failed: {get_status}");

    let body = serde_json::json!({
        "inviteePeer": invitee_peer,
        "expirySecs": 3600u64,
    })
    .to_string();
    eprintln!("t6: POST /a2a/agents/invite-agent/invite body={body}");
    let (status, resp_body) = admin_post(
        host.admin_addr,
        &host.admin_token,
        "/a2a/agents/invite-agent/invite",
        &body,
    )
    .await;
    eprintln!("t6: status={status} body_len={}", resp_body.len());
    assert_eq!(
        status, 200,
        "invite endpoint returned {status}: {resp_body}"
    );

    let resp: serde_json::Value = serde_json::from_str(&resp_body).expect("parse response");
    let invite_val = resp.get("invite").expect("invite field");
    let frame: a2a::InviteFrame =
        serde_json::from_value(invite_val.clone()).expect("parse invite frame");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    verify_invite(&frame, &invitee_peer, now).expect("verify invite frame");
    assert_eq!(frame.payload.invitee_peer, invitee_peer);
    assert_eq!(frame.payload.card.0.payload.agent_id, "invite-agent");

    println!("t6_admin_invite_e2e: PASSED (frame signed, nonce persisted)");
}