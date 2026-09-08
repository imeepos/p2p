//! share_redeem 纯逻辑单测：链接参数错误映射、拒绝码 wire 原文、结果 serde 形状。
//! 拨号全链路见 tests/llm_share_redeem_flow.rs（真 facade Node 互联）。

use super::*;
use std::collections::BTreeMap;

fn peer(byte: u8) -> String {
    bs58::encode([byte; 32]).into_string()
}

#[test]
fn link_param_errors_are_explicit() {
    let bad_scheme = parse_link("https://v1?peer=x&token=abc").expect_err("bad scheme");
    assert!(link_param_error(bad_scheme).contains("scheme 不符"));
    let missing_peer = parse_link(&format!("dsh-llm-share://v1?token={}", "a".repeat(32)))
        .expect_err("missing peer");
    assert!(link_param_error(missing_peer).contains("peer"));
    let missing_token =
        parse_link(&format!("dsh-llm-share://v1?peer={}", peer(1))).expect_err("missing token");
    assert!(link_param_error(missing_token).contains("token"));
    let bad_token = parse_link(&format!("dsh-llm-share://v1?peer={}&token=ZZ", peer(1)))
        .expect_err("bad token");
    assert!(link_param_error(bad_token).contains("token"));
}

#[test]
fn code_str_matches_wire_kebab() {
    assert_eq!(code_str(RedeemCode::ShareRevoked), "share-revoked");
    assert_eq!(code_str(RedeemCode::Expired), "expired");
    assert_eq!(code_str(RedeemCode::Exhausted), "exhausted");
    assert_eq!(code_str(RedeemCode::BoundOther), "bound-other");
    assert_eq!(code_str(RedeemCode::Invalid), "invalid");
}

#[test]
fn rejected_outcome_serializes_without_offer_fields() {
    let outcome = RedeemOutcome {
        status: RedeemStatus::Rejected,
        code: Some("share-revoked".into()),
        offer: None,
        share_id: Some("sid-1".into()),
        owner: None,
    };
    let json = serde_json::to_value(&outcome).unwrap_or_default();
    assert_eq!(json["status"], "rejected");
    assert_eq!(json["code"], "share-revoked");
    assert_eq!(json["shareId"], "sid-1");
    assert!(json.get("offer").is_none());
    assert!(json.get("owner").is_none());
}

#[test]
fn redeemed_outcome_serializes_contract_shape() {
    let outcome = RedeemOutcome {
        status: RedeemStatus::Redeemed,
        code: None,
        offer: Some(RedeemOfferSummary {
            peer: peer(2),
            models: vec!["gpt-4o".into()],
            spare: BTreeMap::from([("gpt-4o".into(), 150)]),
            period_ends: "2026-09-30".into(),
        }),
        share_id: Some("sid-2".into()),
        owner: Some(peer(2)),
    };
    let json = serde_json::to_value(&outcome).unwrap_or_default();
    assert_eq!(json["status"], "redeemed");
    assert_eq!(json["offer"]["peer"], peer(2));
    assert_eq!(json["offer"]["spare"]["gpt-4o"], 150);
    assert_eq!(json["offer"]["periodEnds"], "2026-09-30");
    assert_eq!(json["owner"], peer(2));
    assert!(json.get("code").is_none());
}
