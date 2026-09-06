//! share 模块测试：token 工具、只存哈希、兑换判定优先级、台账原子存取与链接格式。

use std::path::PathBuf;

use uuid::Uuid;

use super::*;
use crate::policy::{AskRoute, Scope};

fn spec(ttl: u64, max: u32) -> ShareSpec {
    ShareSpec {
        scope: Scope::Sandbox,
        allow_mcp: vec!["fs".to_owned()],
        ask_route: AskRoute::RemoteGui,
        max_activations: max,
        note: "nb".to_owned(),
        ttl_secs: ttl,
    }
}

fn entry(ttl: u64, max: u32, token: &str, now: u64) -> ShareEntry {
    ShareEntry::new(spec(ttl, max), token, now, Uuid::new_v4())
}

#[test]
fn token_is_32_lowercase_hex_and_unique() {
    let a = generate_token();
    let b = generate_token();
    assert_eq!(a.len(), 32);
    assert!(a
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_ne!(a, b, "随机 token 两次生成不得相同");
}

#[test]
fn token_sha256_matches_known_vector() {
    assert_eq!(
        token_sha256("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn entry_stores_only_hash_and_derives_fields() {
    let token = generate_token();
    let now = 1_700_000_000;
    let e = entry(3_600, 1, &token, now);
    assert_eq!(e.token_sha256, token_sha256(&token));
    assert!(
        !serde_json::to_string(&e).unwrap().contains(&token),
        "原文不得序列化进台账"
    );
    assert_eq!(e.expires_at_unix, now + 3_600);
    assert_eq!(e.created_at, "2023-11-14T22:13:20Z");
    assert_eq!(e.activations, 0);
    assert_eq!(e.bound_peer, None);
}

#[test]
fn deny_kind_precedence_revoked_expired_reuse_exhausted() {
    let token = generate_token();
    let now = 1_000;
    let mut e = entry(3_600, 1, &token, now);
    assert_eq!(e.deny_kind("peerA", now + 10), None, "新条目可兑换");
    e.revoked = true;
    assert_eq!(
        e.deny_kind("peerA", now + 10),
        Some(ShareDenyKind::Revoked),
        "撤销压过一切"
    );
    e.revoked = false;
    assert_eq!(
        e.deny_kind("peerA", now + 3_600),
        Some(ShareDenyKind::Expired)
    );
    e.bound_peer = Some("peerA".to_owned());
    assert_eq!(e.deny_kind("peerA", now + 10), None, "绑定者本人不受限");
    assert_eq!(
        e.deny_kind("peerB", now + 10),
        Some(ShareDenyKind::Reuse),
        "他人持同 token 即重用拒绝"
    );
    e.activations = 1;
    assert_eq!(
        e.deny_kind("peerA", now + 10),
        Some(ShareDenyKind::Exhausted)
    );
}

#[test]
fn deny_kind_maps_audit_key_and_error_code() {
    let cases = [
        (ShareDenyKind::Reuse, "share-reuse-denied"),
        (ShareDenyKind::Expired, "share-expired"),
        (ShareDenyKind::Revoked, "share-revoked"),
        (ShareDenyKind::Exhausted, "share-exhausted"),
    ];
    for (kind, key) in cases {
        assert_eq!(kind.audit_key(), key);
        assert_eq!(kind.error_code().code(), key);
    }
}

#[test]
fn status_badge_precedence() {
    let now = 1_000;
    let mut e = entry(3_600, 1, &generate_token(), now);
    assert_eq!(e.status(now + 10), "active");
    e.bound_peer = Some("peerA".to_owned());
    assert_eq!(e.status(now + 10), "bound");
    e.activations = 1;
    assert_eq!(e.status(now + 10), "exhausted");
    assert_eq!(e.status(now + 3_600), "expired");
    e.revoked = true;
    assert_eq!(e.status(now + 10), "revoked");
}

fn tmp_path(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("acp-common-share-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("tmp dir");
    dir.join("acp-shares.json")
}

#[test]
fn ledger_save_load_roundtrip_is_atomic() {
    let path = tmp_path("roundtrip");
    let mut ledger = ShareLedger::new();
    ledger.insert(entry(60, 2, &generate_token(), 1_000));
    ledger.insert(entry(60, 1, &generate_token(), 1_000));
    ledger.save(&path).expect("save");
    assert!(
        !path.with_extension("json.tmp").exists(),
        "tmp 文件应已被 rename 消费"
    );
    let loaded = ShareLedger::load(&path).expect("load");
    assert_eq!(loaded, ledger);
    let _ = std::fs::remove_dir_all(path.parent().expect("parent"));
}

#[test]
fn ledger_corrupt_file_is_explicit_error() {
    let path = tmp_path("corrupt");
    std::fs::write(&path, "not json").expect("write");
    let err = ShareLedger::load(&path).expect_err("损坏必须显式报错");
    assert!(matches!(err, ShareStoreError::Corrupted(_)));
    let _ = std::fs::remove_dir_all(path.parent().expect("parent"));
}

#[test]
fn ledger_unsupported_version_is_explicit_error() {
    let path = tmp_path("version");
    std::fs::write(&path, r#"{"version":99,"shares":{}}"#).expect("write");
    let err = ShareLedger::load(&path).expect_err("版本不符必须显式报错");
    assert!(matches!(err, ShareStoreError::UnsupportedVersion(99)));
    let _ = std::fs::remove_dir_all(path.parent().expect("parent"));
}

#[test]
fn share_link_matches_frozen_format() {
    let link = build_share_link(
        "PEER_A",
        &[
            "/ip4/127.0.0.1/udp/4001/quic-v1".to_owned(),
            "/relay/x".to_owned(),
        ],
        "tok123",
        5_000,
        "sid-1",
    );
    assert_eq!(
        link,
        "dsh-acp-share://v1?peer=PEER_A&addr=/ip4/127.0.0.1/udp/4001/quic-v1&addr=/relay/x&token=tok123&exp=5000&sid=sid-1"
    );
}

#[test]
fn rfc3339_known_timestamps() {
    assert_eq!(rfc3339_from_unix(0), "1970-01-01T00:00:00Z");
    assert_eq!(rfc3339_from_unix(1_000_000_000), "2001-09-09T01:46:40Z");
    assert_eq!(rfc3339_from_unix(951_782_400), "2000-02-29T00:00:00Z");
    assert_eq!(rfc3339_from_unix(1_709_164_800), "2024-02-29T00:00:00Z");
}
