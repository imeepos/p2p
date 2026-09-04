use super::*;

fn all_variants() -> Vec<ErrorCode> {
    vec![
        ErrorCode::PeerNotAllowed,
        ErrorCode::LineTooLong { limit_bytes: 1 },
        ErrorCode::FrameTooLarge {
            size_bytes: 2,
            limit_bytes: 1,
        },
        ErrorCode::SubprocessFailed,
        ErrorCode::SessionCapReached { cap: 4 },
        ErrorCode::ConnCapReached { cap: 1 },
        ErrorCode::ReattachTicketInvalid,
        ErrorCode::HandshakeMalformed,
        ErrorCode::NdjsonTruncated,
    ]
}

#[test]
fn wire_codes_are_stable_tokens() {
    let expected = [
        ("peer-not-allowed", ErrorCode::PeerNotAllowed),
        ("line-too-long", ErrorCode::LineTooLong { limit_bytes: 16 }),
        (
            "frame-too-large",
            ErrorCode::FrameTooLarge {
                size_bytes: 2,
                limit_bytes: 1,
            },
        ),
        ("subprocess-failed", ErrorCode::SubprocessFailed),
        (
            "session-cap-reached",
            ErrorCode::SessionCapReached { cap: 4 },
        ),
        ("conn-cap-reached", ErrorCode::ConnCapReached { cap: 1 }),
        ("reattach-ticket-invalid", ErrorCode::ReattachTicketInvalid),
        ("handshake-malformed", ErrorCode::HandshakeMalformed),
        ("ndjson-truncated", ErrorCode::NdjsonTruncated),
    ];
    for (code, variant) in expected {
        assert_eq!(variant.code(), code);
    }
    // 全变体 code/Display 非空（审计面可用）
    for variant in all_variants() {
        assert!(!variant.code().is_empty());
        assert!(variant.to_string().starts_with(variant.code()));
    }
}

#[test]
fn error_trait_impl_present() {
    fn assert_error<T: std::error::Error>() {}
    assert_error::<ErrorCode>();
    let e = ErrorCode::LineTooLong {
        limit_bytes: 16 * 1024 * 1024,
    };
    assert!(
        e.to_string().contains("16777216"),
        "audit display carries limit: {e}"
    );
}
