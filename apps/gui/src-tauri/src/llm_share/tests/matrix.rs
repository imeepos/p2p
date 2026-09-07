//! §16.1 serde roundtrip 矩阵（PR 轨会签映射：serde_* 测试名 ↔ 契约形状条目）：
//! offer 视图 publish/show 形状、status 五态、allowlist entries 形状、borrow 报告
//! done 形状与三态、拒绝码四值 wire 原文、账本/净差/验签结果形状。

use serde_json::json;

use super::common::{json_map, peer};
use crate::llm_share::views::*;

#[test]
fn serde_offer_view_publish_shape_roundtrips() {
    let view = LlmOfferView {
        peer: peer(1),
        models: vec!["gpt-4o".into(), "deepseek-v3".into()],
        spare: json_map(&[("gpt-4o", 1_500_000), ("deepseek-v3", 999_999_999)]),
        period_ends: "2026-09-30".into(),
        max_per_req: json_map(&[("gpt-4o", 128_000)]),
        rate_limit: llm_share_offer::RateLimit {
            rpm: 10,
            concurrency: 2,
        },
        ttl: 3600,
        retention: "none".into(),
        issued_at: 1_788_549_309,
        expires_at: 1_788_552_909,
        file: "/tmp/offer.json".into(),
        remaining_secs: None,
        status: None,
    };
    let value = serde_json::to_value(&view).expect("serialize");
    assert_eq!(
        value,
        json!({
            "peer": peer(1), "models": ["gpt-4o", "deepseek-v3"],
            "spare": {"gpt-4o": 1500000, "deepseek-v3": 999999999},
            "periodEnds": "2026-09-30", "maxPerReq": {"gpt-4o": 128000},
            "rateLimit": {"rpm": 10, "concurrency": 2},
            "ttl": 3600, "retention": "none",
            "issuedAt": 1788549309, "expiresAt": 1788552909, "file": "/tmp/offer.json"
        })
    );
    assert_eq!(serde_json::from_value::<LlmOfferView>(value).unwrap(), view);
}

#[test]
fn serde_offer_status_five_values() {
    for (status, wire) in [
        (LlmOfferStatus::Live, "live"),
        (LlmOfferStatus::Expired, "expired"),
        (LlmOfferStatus::NotYetValid, "not_yet_valid"),
        (LlmOfferStatus::PeerMismatch, "peer_mismatch"),
        (LlmOfferStatus::BadSignature, "bad_signature"),
    ] {
        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            format!("\"{wire}\"")
        );
        assert_eq!(
            serde_json::from_str::<LlmOfferStatus>(&format!("\"{wire}\"")).unwrap(),
            status
        );
    }
}

#[test]
fn serde_offer_view_show_includes_status_and_remaining() {
    let text = r#"{"peer":"a","models":[],"spare":{},"periodEnds":"2026-09-30","maxPerReq":{},"rateLimit":{"rpm":10,"concurrency":2},"ttl":3600,"retention":"none","issuedAt":1,"expiresAt":2,"file":"f","remainingSecs":-3,"status":"expired"}"#;
    let view: LlmOfferView = serde_json::from_str(text).unwrap();
    assert_eq!(view.status, Some(LlmOfferStatus::Expired));
    assert_eq!(view.remaining_secs, Some(-3));
}

#[test]
fn serde_allowlist_entries_shape_roundtrips() {
    let text = r#"{"entries":[{"peerId":"p1","models":["gpt-4o"],"note":"n","grantedAt":"2026-09-04T19:15:09Z"}]}"#;
    let view: LlmAllowlistView = serde_json::from_str(text).unwrap();
    assert_eq!(view.entries[0].peer_id, "p1");
    assert_eq!(
        serde_json::to_value(&view).unwrap(),
        serde_json::from_str::<serde_json::Value>(text).unwrap()
    );
}

#[test]
fn serde_borrow_report_done_shape_roundtrips() {
    let report = LlmBorrowReport {
        status: LlmBorrowStatus::Done,
        receipt: LlmBorrowReceipt {
            req_id: "req-1".into(),
            appended: true,
            estimated: false,
            dispute_window_secs: 86_400,
        },
        sse_count: 9,
        usage: Some(LlmUsage {
            input: 1234,
            output: 567,
        }),
        code: None,
        message: None,
    };
    let value = serde_json::to_value(&report).expect("serialize");
    assert_eq!(
        value,
        json!({
            "status": "done",
            "receipt": {"reqId": "req-1", "appended": true, "estimated": false, "disputeWindowSecs": 86400},
            "sseCount": 9, "usage": {"input": 1234, "output": 567}
        })
    );
    assert_eq!(
        serde_json::from_value::<LlmBorrowReport>(value).unwrap(),
        report
    );
}

#[test]
fn serde_borrow_status_three_values() {
    for (status, wire) in [
        (LlmBorrowStatus::Done, "done"),
        (LlmBorrowStatus::StreamBroken, "stream_broken"),
        (LlmBorrowStatus::Rejected, "rejected"),
    ] {
        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            format!("\"{wire}\"")
        );
    }
}

#[test]
fn serde_reject_codes_four_wire_values_pass_through() {
    // §16.2.1：四拒绝码原样透出不本地化改写（wire snake_case，机器可区分）。
    for code in [
        "not_allowlisted",
        "model_not_served",
        "freeze_insufficient",
        "concurrency_exceeded",
    ] {
        let report = LlmBorrowReport {
            status: LlmBorrowStatus::Rejected,
            receipt: LlmBorrowReceipt {
                req_id: "req-r".into(),
                appended: false,
                estimated: false,
                dispute_window_secs: 86_400,
            },
            sse_count: 0,
            usage: None,
            code: Some(code.to_owned()),
            message: Some("denied".into()),
        };
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["code"], json!(code));
        assert_eq!(value["status"], json!("rejected"));
        assert!(value.get("usage").is_none(), "rejected 无 usage");
        assert_eq!(
            serde_json::from_value::<LlmBorrowReport>(value).unwrap(),
            report
        );
    }
}

#[test]
fn serde_ledger_entry_and_balance_group_shapes() {
    let entry: LlmLedgerEntry = serde_json::from_str(
        r#"{"reqId":"r1","period":"2026-09","lender":"l","borrower":"b","model":"gpt-4o","input":1234,"output":5678,"tokens":6912,"estimated":false,"ts":1788480000}"#,
    )
    .unwrap();
    assert_eq!(entry.tokens, 6912);
    let lent: LlmBalanceGroup = serde_json::from_str(
        r#"{"lender":"l","period":"2026-09","lentOut":40,"borrowed":0,"netAmount":40,"entries":1,"direction":"lent"}"#,
    )
    .unwrap();
    assert_eq!(lent.direction, LlmBalanceDirection::Lent);
    let borrowed: LlmBalanceGroup = serde_json::from_str(
        r#"{"lender":"l","period":"2026-09","lentOut":0,"borrowed":6912,"netAmount":-6912,"entries":1,"direction":"borrowed"}"#,
    )
    .unwrap();
    assert_eq!(borrowed.direction, LlmBalanceDirection::Borrowed);
    // 消费方契约（前端 ledger-balance.tsx 插值字段）：序列化键集逐字断言，防视图漂移
    let wire = serde_json::to_value(&borrowed).unwrap();
    for key in [
        "lender",
        "period",
        "lentOut",
        "borrowed",
        "netAmount",
        "entries",
        "direction",
    ] {
        assert!(wire.get(key).is_some(), "缺消费方字段 {key}");
    }
    assert_eq!(wire["direction"], json!("borrowed"));
    assert_eq!(wire["lentOut"], json!(0));
    assert_eq!(wire["entries"], json!(1));
}

#[test]
fn serde_receipt_verify_result_shape() {
    let text = r#"{"verdict":"PASS","reason":"验签通过","reqId":"r1","period":"2026-09","lender":"l","borrower":"b","model":"gpt-4o","input":1234,"output":5678,"estimated":false,"ts":1788480000}"#;
    let result: LlmReceiptVerifyResult = serde_json::from_str(text).unwrap();
    assert_eq!(result.verdict, "PASS");
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        serde_json::from_str::<serde_json::Value>(text).unwrap()
    );
}

#[test]
fn messages_payload_plain_text_wraps_as_user_message() {
    let wire = crate::llm_share::flows::messages_payload("realflow plain text probe");
    let value: serde_json::Value = serde_json::from_str(&wire).unwrap();
    assert_eq!(
        value,
        json!([{ "role": "user", "content": "realflow plain text probe" }])
    );
}

#[test]
fn messages_payload_array_json_passes_through_unchanged() {
    let raw = r#"[{"role":"user","content":"already-openai"}]"#;
    assert_eq!(crate::llm_share::flows::messages_payload(raw), raw);
}

#[test]
fn messages_payload_non_array_json_still_wraps() {
    let wire = crate::llm_share::flows::messages_payload(r#"{"role":"user"}"#);
    let value: serde_json::Value = serde_json::from_str(&wire).unwrap();
    assert!(value.is_array(), "非数组 JSON 也须包装为消息数组");
}
