use super::*;
use crate::ledger::RedeemError;

#[test]
fn ok_response_roundtrips_without_code_field() {
    let wire = serde_json::to_string(&RedeemResponse::ok()).unwrap();
    assert_eq!(wire, r#"{"ok":true}"#);
    let back: RedeemResponse = serde_json::from_str(&wire).unwrap();
    assert!(back.ok);
    assert_eq!(back.code, None);
}

#[test]
fn deny_serializes_kebab_case_code() {
    let wire = serde_json::to_string(&RedeemResponse::deny(RedeemCode::ShareRevoked)).unwrap();
    assert_eq!(wire, r#"{"ok":false,"code":"share-revoked"}"#);
    let back: RedeemResponse = serde_json::from_str(&wire).unwrap();
    assert!(!back.ok);
    assert_eq!(back.code, Some(RedeemCode::ShareRevoked));
}

#[test]
fn request_roundtrips() {
    let wire = serde_json::to_string(&RedeemRequest {
        token: "a".repeat(32),
    })
    .unwrap();
    assert_eq!(wire, format!(r#"{{"token":"{}"}}"#, "a".repeat(32)));
    let back: RedeemRequest = serde_json::from_str(&wire).unwrap();
    assert_eq!(back.token, "a".repeat(32));
}

#[test]
fn wire_names_match_contract() {
    let cases = [
        (RedeemCode::ShareRevoked, "share-revoked"),
        (RedeemCode::Expired, "expired"),
        (RedeemCode::Exhausted, "exhausted"),
        (RedeemCode::BoundOther, "bound-other"),
        (RedeemCode::Invalid, "invalid"),
    ];
    for (code, want) in cases {
        let wire = serde_json::to_string(&code).unwrap();
        assert_eq!(wire, format!("\"{want}\""));
    }
}

#[test]
fn redeem_error_maps_to_code_and_wire() {
    let cases = [
        (RedeemError::Invalid, RedeemCode::Invalid, "invalid"),
        (
            RedeemError::Revoked,
            RedeemCode::ShareRevoked,
            "share-revoked",
        ),
        (RedeemError::Expired, RedeemCode::Expired, "expired"),
        (RedeemError::Exhausted, RedeemCode::Exhausted, "exhausted"),
        (
            RedeemError::BoundOther,
            RedeemCode::BoundOther,
            "bound-other",
        ),
    ];
    for (err, code, wire) in cases {
        assert_eq!(RedeemCode::from(err), code);
        assert_eq!(err.wire_code(), wire);
    }
}
