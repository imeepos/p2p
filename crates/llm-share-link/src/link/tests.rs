use super::*;

fn sample_token() -> String {
    "a".repeat(32)
}

#[test]
fn roundtrip_preserves_all_fields() {
    let token = sample_token();
    let link = build_link(
        "peer-1",
        &["addr-a".to_owned(), "addr-b".to_owned()],
        &token,
        Some(1_000),
        Some("share-1".to_owned()),
        Some(vec!["gpt-4o".to_owned(), "deepseek-v3".to_owned()]),
    );
    assert!(link.starts_with("dsh-llm-share://v1?peer="), "link: {link}");
    let parsed = parse_link(&link).unwrap();
    assert_eq!(parsed.peer, "peer-1");
    assert_eq!(parsed.addrs, vec!["addr-a", "addr-b"]);
    assert_eq!(parsed.token, token);
    assert_eq!(parsed.exp, Some(1_000));
    assert_eq!(parsed.sid, Some("share-1".to_owned()));
    assert_eq!(
        parsed.models,
        Some(vec!["gpt-4o".to_owned(), "deepseek-v3".to_owned()])
    );
}

#[test]
fn bad_scheme_is_rejected() {
    assert!(matches!(
        parse_link("other://v1?peer=p&token=a"),
        Err(LinkError::BadScheme(_))
    ));
    assert!(matches!(
        parse_link("no-scheme-at-all"),
        Err(LinkError::BadScheme(_))
    ));
}

#[test]
fn missing_peer_or_token_is_rejected() {
    assert_eq!(
        parse_link("dsh-llm-share://v1?token=aaaaaaaa"),
        Err(LinkError::MissingPeer)
    );
    assert_eq!(
        parse_link("dsh-llm-share://v1?peer=p"),
        Err(LinkError::MissingToken)
    );
    assert_eq!(
        parse_link("dsh-llm-share://v1?peer=p&token="),
        Err(LinkError::MissingToken)
    );
    assert_eq!(
        parse_link("dsh-llm-share://v1?peer=&token=aaaaaaaa"),
        Err(LinkError::MissingPeer)
    );
}

#[test]
fn bad_token_is_rejected() {
    let t = sample_token();
    let uppercase = t.to_uppercase();
    assert!(matches!(
        parse_link(&format!("dsh-llm-share://v1?peer=p&token={uppercase}")),
        Err(LinkError::BadToken(_))
    ));
    let non_hex = "z".repeat(32);
    assert!(matches!(
        parse_link(&format!("dsh-llm-share://v1?peer=p&token={non_hex}")),
        Err(LinkError::BadToken(_))
    ));
    let short = "a".repeat(31);
    assert!(matches!(
        parse_link(&format!("dsh-llm-share://v1?peer=p&token={short}")),
        Err(LinkError::BadToken(_))
    ));
}

#[test]
fn unknown_params_are_ignored() {
    let link = format!(
        "dsh-llm-share://v1?peer=p&token={}&extra=x&flag",
        sample_token()
    );
    let parsed = parse_link(&link).unwrap();
    assert_eq!(parsed.peer, "p");
}

#[test]
fn malformed_exp_falls_back_to_none() {
    let link = format!(
        "dsh-llm-share://v1?peer=p&token={}&exp=not-a-number",
        sample_token()
    );
    let parsed = parse_link(&link).unwrap();
    assert_eq!(parsed.exp, None);
}

#[test]
fn empty_models_means_none() {
    let link = format!("dsh-llm-share://v1?peer=p&token={}&models=", sample_token());
    let parsed = parse_link(&link).unwrap();
    assert_eq!(parsed.models, None);
}

#[test]
fn token_validation_bounds() {
    assert!(is_valid_token(&"a".repeat(32)));
    assert!(!is_valid_token(&"A".repeat(32)));
    assert!(!is_valid_token(&"a".repeat(31)));
    assert!(!is_valid_token(&"a".repeat(33)));
    assert!(!is_valid_token(&"g".repeat(32)));
    assert!(!is_valid_token(""));
}

#[test]
fn build_link_omits_optional_none_fields() {
    let link = build_link("p", &[], &sample_token(), None, None, None);
    assert_eq!(
        link,
        format!("dsh-llm-share://v1?peer=p&token={}", sample_token())
    );
    assert!(!link.contains("exp="));
    assert!(!link.contains("sid="));
    assert!(!link.contains("models="));
}
