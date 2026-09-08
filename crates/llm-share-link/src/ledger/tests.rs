use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::*;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("llm-link-ledger-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn token(byte: u8) -> String {
    format!("{byte:02x}").repeat(16)
}

#[test]
fn lifecycle_create_redeem_and_status() {
    let dir = temp_dir("lifecycle");
    let path = dir.join(FILE_NAME);
    let mut ledger = ShareLedger::new();
    let share_id = ledger.create(&token(1), "p1", vec!["m1".into()], 1_000_000, "note", 100);
    ledger.save(&path).unwrap();
    assert!(!path.with_extension("json.tmp").exists());
    let loaded = ShareLedger::load_or_empty(&path).unwrap();
    let entry = loaded.get(&share_id).unwrap();
    assert_eq!(entry.status(1_000), "active");
    assert_eq!(entry.token_sha256, token_sha256(&token(1)));
    assert_ne!(entry.token_sha256, token(1), "台账只存摘要");

    let mut redeeming = loaded;
    let result = redeeming.redeem(&token(1), "peer-a", 1_000).unwrap();
    assert_eq!(result.peer, "peer-a");
    assert_eq!(result.source, format!("{SOURCE_PREFIX}{share_id}"));
    assert_eq!(result.models, vec!["m1"]);
    let entry = redeeming.get(&share_id).unwrap();
    assert_eq!(entry.status(1_000), "exhausted", "激活 1 次即耗尽");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn same_peer_second_redeem_is_idempotent_no_count() {
    let mut ledger = ShareLedger::new();
    ledger.create(&token(2), "p1", vec![], 1_000_000, "", 100);
    assert!(ledger.redeem(&token(2), "peer-a", 1_000).is_ok());
    assert!(
        ledger.redeem(&token(2), "peer-a", 1_000).is_ok(),
        "同 peer 幂等成功"
    );
    let entry = ledger.iter().next().unwrap().1;
    assert_eq!(entry.activations, 1, "幂等二次不计数");
    assert_eq!(entry.bound_peer.as_deref(), Some("peer-a"));
}

#[test]
fn different_peer_after_bind_is_bound_other() {
    let mut ledger = ShareLedger::new();
    ledger.create(&token(3), "p1", vec![], 1_000_000, "", 100);
    assert!(ledger.redeem(&token(3), "peer-a", 1_000).is_ok());
    assert_eq!(
        ledger.redeem(&token(3), "peer-b", 1_000),
        Err(RedeemError::BoundOther)
    );
    assert_eq!(
        ledger.redeem(&token(4), "peer-a", 1_000),
        Err(RedeemError::Invalid)
    );
}

#[test]
fn concurrent_redeem_same_token_activates_once() {
    let mut ledger = ShareLedger::new();
    let share_id = ledger.create(&token(5), "p1", vec!["m1".into()], 1_000_000, "", 100);
    let shared = Arc::new(Mutex::new(ledger));
    let t = token(5);
    let h1 = {
        let shared = shared.clone();
        let t = t.clone();
        std::thread::spawn(move || shared.lock().unwrap().redeem(&t, "peer-a", 1_000).is_ok())
    };
    let h2 = {
        let shared = shared.clone();
        let t = t.clone();
        std::thread::spawn(move || shared.lock().unwrap().redeem(&t, "peer-b", 1_000).is_ok())
    };
    let oks = [h1.join().unwrap(), h2.join().unwrap()]
        .iter()
        .filter(|ok| **ok)
        .count();
    assert_eq!(oks, 1, "双线程同 token 只一激活");
    let guard = shared.lock().unwrap();
    let entry = guard.get(&share_id).unwrap();
    assert_eq!(entry.activations, 1);
    assert!(entry.bound_peer.is_some());
}

#[test]
fn revoke_then_redeem_rejected() {
    let mut ledger = ShareLedger::new();
    let share_id = ledger.create(&token(6), "p1", vec![], 1_000_000, "", 100);
    assert!(ledger.revoke(&share_id));
    assert_eq!(
        ledger.redeem(&token(6), "peer-a", 1_000),
        Err(RedeemError::Revoked)
    );
    assert!(!ledger.revoke("nonexistent"));
}

#[test]
fn expired_share_redeem_rejected() {
    let mut ledger = ShareLedger::new();
    let share_id = ledger.create(&token(7), "p1", vec![], 1_000, "", 100);
    assert_eq!(
        ledger.redeem(&token(7), "peer-a", 1_000),
        Err(RedeemError::Expired)
    );
    let result = ledger.redeem(&token(7), "peer-a", 999).unwrap();
    assert_eq!(
        result.source,
        format!("{SOURCE_PREFIX}{share_id}"),
        "未到期应可兑换"
    );
}

#[test]
fn corrupt_file_is_explicit_error() {
    let dir = temp_dir("corrupt");
    let path = dir.join(FILE_NAME);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&path, "{ not json").unwrap();
    assert!(matches!(
        ShareLedger::load_or_empty(&path),
        Err(LedgerError::Corrupted { .. })
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_file_loads_empty() {
    let dir = temp_dir("missing");
    let ledger = ShareLedger::load_or_empty(&dir.join(FILE_NAME)).unwrap();
    assert!(ledger.iter().next().is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unsupported_version_is_explicit_error() {
    let dir = temp_dir("version");
    let path = dir.join(FILE_NAME);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&path, r#"{"v":2,"entries":{}}"#).unwrap();
    assert!(matches!(
        ShareLedger::load_or_empty(&path),
        Err(LedgerError::UnsupportedVersion(2))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
