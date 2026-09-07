use std::path::{Path, PathBuf};

use super::*;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2pcli-share-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn peer() -> String {
    bs58::encode([7u8; 32]).into_string()
}

/// 装配 provider（prov-a，模型 gpt-4o）+ offer（gpt-4o）。
fn setup(dir: &Path) {
    provider::save(
        dir.to_str().unwrap(),
        provider::SaveParams {
            id: Some("prov-a".to_owned()),
            name: "A".to_owned(),
            base_url: "https://a.example/v1".to_owned(),
            protocol: provider::Protocol::OpenAI,
            api_key: "sk-a-1234567890".to_owned(),
            models: vec!["gpt-4o".to_owned()],
            created_at: 1_000,
        },
    )
    .unwrap();
    let seed = dir.join("key.seed");
    p2p_identity::save_seed(&seed, &p2p_identity::Keypair::generate()).unwrap();
    let params = offer::OfferParams {
        models: vec!["gpt-4o".to_owned()],
        spare: vec!["gpt-4o=1500000".to_owned()],
        period_ends: "2026-09-30".to_owned(),
        max_per_req: vec![],
        rpm: 10,
        concurrency: 2,
        ttl_secs: 3600,
        retention: None,
    };
    offer::publish(&seed, dir.to_str().unwrap(), &params, 1_000).unwrap();
}

fn create_params(provider_id: &str, models: Option<Vec<String>>) -> ShareCreateParams {
    ShareCreateParams {
        peer: peer(),
        provider_id: provider_id.to_owned(),
        models,
        expires_at_unix: None,
        note: "n".to_owned(),
        addrs: vec!["127.0.0.1/u52063".to_owned()],
    }
}

#[test]
fn create_requires_published_offer() {
    let dir = temp_dir("nooffer");
    provider::save(
        dir.to_str().unwrap(),
        provider::SaveParams {
            id: Some("prov-a".to_owned()),
            name: "A".to_owned(),
            base_url: "https://a.example/v1".to_owned(),
            protocol: provider::Protocol::OpenAI,
            api_key: "sk-a-1234567890".to_owned(),
            models: vec!["gpt-4o".to_owned()],
            created_at: 1_000,
        },
    )
    .unwrap();
    let err = share_create(dir.to_str().unwrap(), create_params("prov-a", None), 1_000).unwrap_err();
    assert!(err.contains("offer publish"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_requires_existing_provider() {
    let dir = temp_dir("noprovider");
    setup(&dir);
    let err = share_create(dir.to_str().unwrap(), create_params("missing", None), 1_000).unwrap_err();
    assert!(err.contains("provider 不存在"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_rejects_model_outside_offer() {
    let dir = temp_dir("modeloutside");
    setup(&dir);
    let err = share_create(
        dir.to_str().unwrap(),
        create_params("prov-a", Some(vec!["claude-3".to_owned()])),
        1_000,
    )
    .unwrap_err();
    assert!(err.contains("不在当前能力声明中"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_exp_bounds() {
    let dir = temp_dir("expbounds");
    setup(&dir);
    let exp_equal_now = ShareCreateParams {
        expires_at_unix: Some(1_000),
        ..create_params("prov-a", None)
    };
    assert!(share_create(dir.to_str().unwrap(), exp_equal_now, 1_000).is_err());
    let exp_over_max = ShareCreateParams {
        expires_at_unix: Some(1_000 + MAX_TTL_SECS + 1),
        ..create_params("prov-a", None)
    };
    assert!(share_create(dir.to_str().unwrap(), exp_over_max, 1_000).is_err());
    let exp_at_max = ShareCreateParams {
        expires_at_unix: Some(1_000 + MAX_TTL_SECS),
        ..create_params("prov-a", None)
    };
    let report = share_create(dir.to_str().unwrap(), exp_at_max, 1_000).unwrap();
    assert_eq!(report.expires_at, 1_000 + MAX_TTL_SECS);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_default_ttl_and_link_roundtrip() {
    let dir = temp_dir("roundtrip");
    setup(&dir);
    let report = share_create(dir.to_str().unwrap(), create_params("prov-a", None), 1_000).unwrap();
    assert_eq!(report.expires_at, 1_000 + DEFAULT_TTL_SECS);
    assert_eq!(report.models, vec!["gpt-4o"]);
    let parsed = llm_share_link::link::parse_link(&report.link).unwrap();
    assert_eq!(parsed.peer, peer());
    assert_eq!(parsed.sid.as_deref(), Some(report.share_id.as_str()));
    assert_eq!(parsed.exp, Some(report.expires_at));
    assert_eq!(parsed.models, Some(vec!["gpt-4o".to_owned()]));
    assert_eq!(parsed.addrs, vec!["127.0.0.1/u52063"]);
    let ledger = ShareLedger::load_or_empty(&ledger_path(dir.to_str().unwrap())).unwrap();
    let entry = ledger.get(&report.share_id).unwrap();
    assert_eq!(entry.token_sha256, llm_share_link::token::token_sha256(&parsed.token));
    assert!(!report.link.contains(&parsed.token) || parsed.token.len() == 32);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn list_reports_statuses() {
    let dir = temp_dir("liststatus");
    setup(&dir);
    let report = share_create(dir.to_str().unwrap(), create_params("prov-a", None), 1_000).unwrap();
    let listed = share_list(dir.to_str().unwrap(), 1_500).unwrap();
    assert_eq!(listed.shares.len(), 1);
    assert_eq!(listed.shares[0].status, "active");
    assert_eq!(listed.shares[0].share_id, report.share_id);
    let expired_view = share_list(dir.to_str().unwrap(), report.expires_at).unwrap();
    assert_eq!(expired_view.shares[0].status, "expired");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revoke_cascades_allowlist_and_repeat_revoke_errors() {
    let dir = temp_dir("revoke");
    setup(&dir);
    let report = share_create(dir.to_str().unwrap(), create_params("prov-a", None), 1_000).unwrap();
    let source = format!("{SOURCE_PREFIX}{}", report.share_id);
    allowlist::allow(
        dir.to_str().unwrap(),
        &peer(),
        &["gpt-4o".to_owned()],
        None,
        Some(&source),
        Some(report.expires_at),
        "t",
    )
    .unwrap();
    let revoked = share_revoke(dir.to_str().unwrap(), &report.share_id).unwrap();
    assert!(revoked.revoked);
    assert_eq!(revoked.allowlist_removed, 1);
    let listed = allowlist::list(dir.to_str().unwrap()).unwrap();
    assert!(listed.peers.is_empty(), "share 来源条目被级联删除");
    assert!(share_revoke(dir.to_str().unwrap(), &report.share_id).is_err(), "重复撤销显式报错");
    assert!(share_revoke(dir.to_str().unwrap(), "nonexistent").is_err(), "不存在显式报错");
    let _ = std::fs::remove_dir_all(&dir);
}