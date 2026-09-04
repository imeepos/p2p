use super::*;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2pcli-offer-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn params() -> OfferParams {
    OfferParams {
        models: vec!["gpt-4o".to_owned()],
        spare: vec!["gpt-4o=1500000".to_owned()],
        period_ends: "2026-09-30".to_owned(),
        max_per_req: vec!["gpt-4o=128000".to_owned()],
        rpm: 10,
        concurrency: 2,
        ttl_secs: 3600,
        retention: None,
    }
}

#[test]
fn publish_signs_and_show_reports_live() {
    let dir = temp_dir("live");
    let seed = dir.join("key.seed");
    let kp = p2p_identity::Keypair::generate();
    p2p_identity::save_seed(&seed, &kp).unwrap();
    let report = publish(&seed, dir.to_str().unwrap(), &params(), 1_000).unwrap();
    assert_eq!(report.issued_at, 1_000);
    assert_eq!(report.expires_at, 1_000 + 3600);
    assert_eq!(report.retention, "none");
    let view = show(dir.to_str().unwrap(), 1_500).unwrap();
    assert_eq!(view.status, "live");
    assert_eq!(view.remaining_secs, 3100);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn publish_without_seed_is_explicit_error() {
    let dir = temp_dir("noseed");
    let err = publish(
        &dir.join("absent.seed"),
        dir.to_str().unwrap(),
        &params(),
        1,
    )
    .unwrap_err();
    assert!(err.contains("节点身份加载失败"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn expired_show_reports_expired_status() {
    let dir = temp_dir("expired");
    let seed = dir.join("key.seed");
    p2p_identity::save_seed(&seed, &p2p_identity::Keypair::generate()).unwrap();
    publish(&seed, dir.to_str().unwrap(), &params(), 1_000).unwrap();
    let view = show(dir.to_str().unwrap(), 1_000 + 3600).unwrap();
    assert_eq!(view.status, "expired");
    assert_eq!(view.remaining_secs, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tampered_store_show_flags_bad_signature() {
    let dir = temp_dir("tamper");
    let seed = dir.join("key.seed");
    p2p_identity::save_seed(&seed, &p2p_identity::Keypair::generate()).unwrap();
    publish(&seed, dir.to_str().unwrap(), &params(), 1_000).unwrap();
    let file = path(dir.to_str().unwrap());
    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    value["offer"]["spare"]["gpt-4o"] = serde_json::json!(999);
    std::fs::write(&file, serde_json::to_string(&value).unwrap()).unwrap();
    assert_eq!(
        show(dir.to_str().unwrap(), 1_500).unwrap().status,
        "bad_signature"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn build_offer_rejects_invalid_params() {
    let dir = temp_dir("invalid");
    let seed = dir.join("key.seed");
    p2p_identity::save_seed(&seed, &p2p_identity::Keypair::generate()).unwrap();
    let mut bad = params();
    bad.models.clear();
    assert!(publish(&seed, dir.to_str().unwrap(), &bad, 1).is_err());
    let mut bad = params();
    bad.period_ends = "2026/09/30".into();
    assert!(publish(&seed, dir.to_str().unwrap(), &bad, 1).is_err());
    let mut bad = params();
    bad.spare.clear();
    let err = publish(&seed, dir.to_str().unwrap(), &bad, 1).unwrap_err();
    assert!(err.contains("gpt-4o"), "{err}");
    let mut bad = params();
    bad.spare = vec!["unknown-model=5".to_owned()];
    assert!(publish(&seed, dir.to_str().unwrap(), &bad, 1).is_err());
    let mut bad = params();
    bad.spare = vec!["gpt-4o=1".to_owned(), "gpt-4o=2".to_owned()];
    assert!(publish(&seed, dir.to_str().unwrap(), &bad, 1).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn show_missing_offer_is_explicit_error() {
    let dir = temp_dir("noshow");
    assert!(show(dir.to_str().unwrap(), 1).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
