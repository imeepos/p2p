//! 验收单测：验签篡改拒绝 / req_id 幂等 / 账期切分与双边符号 / 哈希链对账（MVP A3/A4）。

use llm_share_ledger::{Error, Ledger, Receipt, Usage};
use p2p_identity::Keypair;

fn kp(seed: u8) -> Keypair {
    Keypair::from_seed(&[seed; 32])
}

fn peer(k: &Keypair) -> String {
    k.peer_id().to_string()
}

fn receipt(req_id: &str, lender: &str, borrower: &str, period: &str, total: u64) -> Receipt {
    Receipt {
        v: 1,
        req_id: req_id.to_string(),
        period: period.to_string(),
        lender: lender.to_string(),
        borrower: borrower.to_string(),
        model: "gpt-4o".to_string(),
        usage: Usage { input: total / 2, output: total - total / 2 },
        estimated: false,
        upstream_hint: "openai".into(),
        ts: 1_725_400_000,
        sig: String::new(),
    }
}

#[test]
fn verify_accepts_signed_and_rejects_tampered() {
    let lender = kp(9);
    let mut r = receipt("req-1", &peer(&lender), "bob", "2026-09", 100);
    r.sign(&lender).unwrap();
    assert!(r.verify(&lender.public()).is_ok());

    // 篡改 usage 后验签必失败（A3）。
    let mut tampered = r.clone();
    tampered.usage.output += 1;
    assert!(matches!(tampered.verify(&lender.public()), Err(Error::BadSignature(_))));

    // 篡改其他字段、换公钥、破坏签名编码，同样拒绝。
    let mut shifted = r.clone();
    shifted.ts += 1;
    assert!(shifted.verify(&lender.public()).is_err());
    assert!(r.verify(&kp(11).public()).is_err());
    let mut broken = r.clone();
    broken.sig = "not-base58!!".into();
    assert!(matches!(broken.verify(&lender.public()), Err(Error::Malformed(_))));
}

#[test]
fn req_id_replay_records_single_entry() {
    let lender = kp(9);
    let mut r = receipt("req-1", &peer(&lender), "bob", "2026-09", 100);
    r.sign(&lender).unwrap();
    let mut ledger = Ledger::default();
    assert!(ledger.apply(&r, &lender.public()).unwrap());
    // 重放只记一笔（A4）。
    assert!(!ledger.apply(&r, &lender.public()).unwrap());
    assert_eq!(ledger.net(&peer(&lender), "2026-09"), 100);

    // 同 req_id 的伪造收据：验签先于去重，直接拒绝。
    let mut forged = r.clone();
    forged.usage.output += 5;
    assert!(matches!(ledger.apply(&forged, &lender.public()), Err(Error::BadSignature(_))));
    assert_eq!(ledger.net(&peer(&lender), "2026-09"), 100);

    let mut r2 = receipt("req-2", &peer(&lender), "bob", "2026-09", 30);
    r2.sign(&lender).unwrap();
    assert!(ledger.apply(&r2, &lender.public()).unwrap());
    assert_eq!(ledger.net(&peer(&lender), "2026-09"), 130);
}

#[test]
fn net_view_splits_by_lender_and_period() {
    let a = kp(1);
    let b = kp(2);
    let c = kp(3);
    let mut ledger = Ledger::default();
    for (req, lender, period, total) in
        [("r1", &a, "2026-09", 100), ("r2", &a, "2026-10", 50), ("r3", &b, "2026-09", 30)]
    {
        let mut r = receipt(req, &peer(lender), &peer(&c), period, total);
        r.sign(lender).unwrap();
        assert!(ledger.apply(&r, &lender.public()).unwrap());
    }
    // 出借方视角：净差按 lender+period 二元组切（轮 52 Q1）。
    assert_eq!(ledger.net(&peer(&a), "2026-09"), 100);
    assert_eq!(ledger.net(&peer(&a), "2026-10"), 50);
    assert_eq!(ledger.net(&peer(&b), "2026-09"), 30);
    // 借方视角为负（双边记账：借方负/贷方正），跨出借方汇总为余额视图。
    assert_eq!(ledger.net(&peer(&c), "2026-09"), -130);
    assert_eq!(ledger.net(&peer(&c), "2026-10"), -50);
    assert_eq!(ledger.net(&peer(&kp(4)), "2026-09"), 0);
}

#[test]
fn reconcile_agrees_and_locates_divergence() {
    let lender = kp(9);
    let mut local = Ledger::default();
    let mut remote = Ledger::default();
    for req in ["r1", "r2"] {
        let mut r = receipt(req, &peer(&lender), "bob", "2026-09", 40);
        r.sign(&lender).unwrap();
        local.apply(&r, &lender.public()).unwrap();
        remote.apply(&r, &lender.public()).unwrap();
    }
    let report = local.reconcile(&remote);
    assert!(report.local_only.is_empty() && report.remote_only.is_empty());
    assert_eq!(report.matched, 2);

    // 本侧多一条：定位缺失。
    let mut only_local = receipt("r3", &peer(&lender), "bob", "2026-09", 10);
    only_local.sign(&lender).unwrap();
    local.apply(&only_local, &lender.public()).unwrap();
    let report = local.reconcile(&remote);
    assert!(!report.local_only.is_empty());
    assert_eq!(report.matched, 2);
    assert_eq!(report.local_only, vec![2]);
    assert!(report.remote_only.is_empty());

    // 同 seq 两侧条目不同：双侧都报告分叉 seq。
    let mut diverged = receipt("r4", &peer(&lender), "bob", "2026-09", 20);
    diverged.sign(&lender).unwrap();
    remote.apply(&diverged, &lender.public()).unwrap();
    let report = local.reconcile(&remote);
    assert_eq!(report.matched, 2);
    assert_eq!(report.local_only, vec![2]);
    assert_eq!(report.remote_only, vec![2]);
}
