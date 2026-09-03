//! 票据校验矩阵（remote-support-plan.md §3.3）：坏签名/过期/scope 非法/
//! 对端不匹配/重复 ticket 各一测 + mint->verify roundtrip + 过期即焚。
//! 纯逻辑测试：确定性时钟与固定种子，不起网络。

use crate::ticket::{
    mint, parse_peer_id, TicketError, TicketLedger, TicketVerifier, SCOPE_DIAG, SCOPE_FIX,
};
use base64::Engine as _;
use p2p_identity::{Keypair, PeerId};

fn platform() -> Keypair {
    Keypair::from_seed(&[7u8; 32])
}

fn other_key(tag: u8) -> Keypair {
    Keypair::from_seed(&[tag; 32])
}

fn helper_peer() -> PeerId {
    other_key(1).peer_id()
}

fn bridge_peer() -> PeerId {
    other_key(2).peer_id()
}

fn other_peer() -> PeerId {
    other_key(3).peer_id()
}

fn verifier() -> TicketVerifier {
    TicketVerifier::new(platform().public(), TicketLedger::default())
}

fn fresh_ticket(now: u64) -> String {
    mint(
        &platform(),
        "t-1",
        &helper_peer().to_string(),
        &bridge_peer().to_string(),
        SCOPE_DIAG,
        3600,
        now,
    )
    .unwrap()
}

#[test]
fn mint_verify_roundtrip_passes() {
    let now = 1_700_000_000u64;
    let ticket = fresh_ticket(now);
    let payload = verifier()
        .verify(&ticket, &bridge_peer(), now + 10)
        .unwrap();
    assert_eq!(payload.ticket_id, "t-1");
    assert_eq!(payload.helper_peer, helper_peer().to_string());
    assert_eq!(payload.bridge_peer, bridge_peer().to_string());
    assert_eq!(payload.scope, SCOPE_DIAG);
    assert_eq!(payload.iat, now);
    assert_eq!(payload.exp, now + 3600);
}

#[test]
fn bad_signature_rejected() {
    let now = 1_700_000_000u64;
    let ticket = fresh_ticket(now);
    // 篡改签名段中部一个字符（末位受 base64url 无填充低位约束，中部不受限）
    let (body, sig) = ticket.split_once('.').unwrap();
    let idx = sig.len() / 2;
    let flipped = if sig.as_bytes()[idx] == b'A' {
        'B'
    } else {
        'A'
    };
    let tampered = format!("{}.{}{}{}", body, &sig[..idx], flipped, &sig[idx + 1..]);
    assert_eq!(
        verifier().verify(&tampered, &bridge_peer(), now + 10),
        Err(TicketError::BadSignature)
    );
}

#[test]
fn expired_ticket_rejected() {
    let now = 1_700_000_000u64;
    let ticket = fresh_ticket(now); // exp = now + 3600
    assert_eq!(
        verifier().verify(&ticket, &bridge_peer(), now + 3600),
        Err(TicketError::Expired)
    );
    assert_eq!(
        verifier().verify(&ticket, &bridge_peer(), now + 86_400),
        Err(TicketError::Expired)
    );
}

#[test]
fn illegal_scope_rejected() {
    let now = 1_700_000_000u64;
    let ticket = mint(
        &platform(),
        "t-scope",
        &helper_peer().to_string(),
        &bridge_peer().to_string(),
        "admin",
        3600,
        now,
    );
    assert_eq!(ticket, Err(TicketError::BadScope("admin".to_string())));
    // 绕过 mint 校验直接构造非法 scope 票据，verify 必须同样拒绝
    let payload = serde_json::json!({
        "ticket_id": "t-scope-2",
        "helper_peer": helper_peer().to_string(),
        "bridge_peer": bridge_peer().to_string(),
        "scope": "write",
        "iat": now,
        "exp": now + 3600,
    });
    let body = serde_json::to_vec(&payload).unwrap();
    let sig = platform().sign(&body);
    let raw = format!(
        "{}.{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&body),
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sig)
    );
    assert_eq!(
        verifier().verify(&raw, &bridge_peer(), now + 10),
        Err(TicketError::BadScope("write".to_string()))
    );
}

#[test]
fn inbound_peer_mismatch_rejected() {
    let now = 1_700_000_000u64;
    let ticket = fresh_ticket(now);
    assert_eq!(
        verifier().verify(&ticket, &other_peer(), now + 10),
        Err(TicketError::PeerMismatch {
            bridge_peer: bridge_peer().to_string()
        })
    );
}

#[test]
fn duplicate_ticket_rejected_once() {
    let now = 1_700_000_000u64;
    let ticket = fresh_ticket(now);
    let verify = verifier();
    assert!(verify.verify(&ticket, &bridge_peer(), now + 10).is_ok());
    assert_eq!(
        verify.verify(&ticket, &bridge_peer(), now + 20),
        Err(TicketError::AlreadyUsed)
    );
}

#[test]
fn same_signature_rejected_when_platform_key_differs() {
    // 换个平台密钥签发与验签，签名验真必须失败
    let now = 1_700_000_000u64;
    let ticket = mint(
        &other_key(9),
        "t-other",
        &helper_peer().to_string(),
        &bridge_peer().to_string(),
        SCOPE_FIX,
        3600,
        now,
    )
    .unwrap();
    assert_eq!(
        verifier().verify(&ticket, &bridge_peer(), now + 10),
        Err(TicketError::BadSignature)
    );
}

#[test]
fn parse_peer_id_accepts_base58_and_rejects_bad() {
    assert_eq!(
        parse_peer_id(&bridge_peer().to_string()).unwrap(),
        bridge_peer()
    );
    assert!(parse_peer_id("not-base58!").is_err());
    assert!(parse_peer_id("1111").is_err());
}

#[test]
fn expired_entry_burns_in_ledger_before_reclaim() {
    let now = 1_700_000_000u64;
    let ledger = TicketLedger::default();
    let verify = TicketVerifier::new(platform().public(), ledger.clone());
    let ticket = fresh_ticket(now); // exp = now + 3600
    verify.verify(&ticket, &bridge_peer(), now + 10).unwrap();
    assert_eq!(ledger.len(), 1);
    // 过期后同 id 票据本身已过期：verify 在查重之前即因 Expired 拒绝（一票否决）
    assert_eq!(
        verify.verify(&ticket, &bridge_peer(), now + 3600),
        Err(TicketError::Expired)
    );
    // 焚毁是惰性的：后续登记触发清理，过期条目随之下表（过期即焚）
    let replacement = mint(
        &platform(),
        "t-new",
        &helper_peer().to_string(),
        &bridge_peer().to_string(),
        SCOPE_DIAG,
        3600,
        now + 3600,
    )
    .unwrap();
    assert!(verify
        .verify(&replacement, &bridge_peer(), now + 3600)
        .is_ok());
    assert_eq!(ledger.len(), 1); // t-1 已焚毁，仅剩 t-new
}
