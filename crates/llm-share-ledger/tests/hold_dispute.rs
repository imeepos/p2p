//! 验收单测：冻结硬闸 / 净差上限结构化拒绝 / estimated 72h 争议状态机（MVP A2/A6）。

use llm_share_ledger::{
    DisputeTracker, Error, FreezeRequest, HoldManager, Ledger, LimitPolicy, Receipt, Usage,
    WINDOW_ESTIMATED_SECS, WINDOW_SECS,
};
use p2p_identity::Keypair;

fn kp(seed: u8) -> Keypair {
    Keypair::from_seed(&[seed; 32])
}

fn peer(k: &Keypair) -> String {
    k.peer_id().to_string()
}

fn req(req_id: &str, lender: &str, est: u64) -> FreezeRequest {
    FreezeRequest {
        req_id: req_id.to_string(),
        lender: lender.to_string(),
        borrower: "bob".to_string(),
        period: "2026-09".to_string(),
        est,
    }
}

#[test]
fn freeze_is_hard_gate_for_settlement() {
    let lender = peer(&kp(9));
    let mut holds = HoldManager::default();
    let limit = LimitPolicy::default().limit(1000);
    assert_eq!(limit, 500);

    holds.freeze(limit, 0, req("r1", &lender, 400)).unwrap();

    // 在途冻结计入投影：400 + 200 = 600 > 500，结构化拒绝且上游零调用（A2）。
    assert!(matches!(
        holds.freeze(limit, 0, req("r2", &lender, 200)),
        Err(Error::NetDiffExceeded(500, 600))
    ));

    // req_id 重试幂等：不重复占额、不拒绝。
    holds.freeze(limit, 0, req("r1", &lender, 400)).unwrap();

    // 硬闸：无冻结不可结算。
    assert!(matches!(holds.settle("r-x", 1), Err(Error::NotFound(_))));
    // usage 超 est：拒绝且冻结保留，闸门仍在。
    assert!(matches!(holds.settle("r1", 401), Err(Error::HoldInsufficient(400, 401))));
    assert!(matches!(
        holds.freeze(limit, 0, req("r2", &lender, 200)),
        Err(Error::NetDiffExceeded(_, _))
    ));
    assert!(holds.settle("r1", 400).is_ok());
    assert!(holds.settle("r1", 1).is_err());

    // release 消费冻结；对已消费 req_id 释放为 NotFound。
    holds.freeze(limit, 0, req("r3", &lender, 100)).unwrap();
    assert!(holds.release("r3").is_ok());
    assert!(matches!(holds.release("r3"), Err(Error::NotFound(_))));
}

#[test]
fn net_diff_limit_counts_net_and_cap() {
    let policy = LimitPolicy::default();
    let mut holds = HoldManager::default();
    let lender = peer(&kp(9));

    // 账面净差 -300（此前 B 多出借）：投影 700 - 300 = 400 ≤ 500，放行。
    assert!(holds.freeze(policy.limit(1000), -300, req("r1", &lender, 700)).is_ok());
    holds.release("r1").unwrap();

    // 绝对封顶：min(50% * 1000, 200) = 200，负净差同样救不回。
    let capped = LimitPolicy { spare_ratio_bps: 5000, absolute_cap: Some(200) };
    assert_eq!(capped.limit(1000), 200);
    assert!(matches!(
        holds.freeze(capped.limit(1000), -300, req("r2", &lender, 700)),
        Err(Error::NetDiffExceeded(200, 400))
    ));

    // 零估算拒绝：冻结必须表达真实风险敞口。
    assert!(matches!(
        holds.freeze(policy.limit(1000), 0, req("r3", &lender, 0)),
        Err(Error::NetDiffExceeded(_, _))
    ));
}

#[test]
fn freeze_settle_books_ledger_via_receipt() {
    let lender = kp(9);
    let lender_id = peer(&lender);
    let mut ledger = Ledger::default();
    let mut holds = HoldManager::default();
    let limit = LimitPolicy::default().limit(1000);

    let fr = req("r1", &lender_id, 500);
    holds.freeze(limit, ledger.net(&lender_id, "2026-09"), fr.clone()).unwrap();
    holds.settle("r1", 380).unwrap();

    let mut r = Receipt {
        v: 1,
        req_id: fr.req_id,
        period: fr.period,
        lender: fr.lender,
        borrower: fr.borrower,
        model: "gpt-4o".to_string(),
        usage: Usage { input: 100, output: 280 },
        estimated: false,
        upstream_hint: "openai".to_string(),
        ts: 42,
        sig: String::new(),
    };
    r.sign(&lender).unwrap();
    assert!(ledger.apply(&r, &lender.public()).unwrap());
    assert_eq!(ledger.net(&lender_id, "2026-09"), 380);
    // 同一收据重复入账被幂等吸收。
    assert!(!ledger.apply(&r, &lender.public()).unwrap());
}

#[test]
fn estimated_receipt_has_72h_dispute_window() {
    let t0 = 1_725_400_000u64;
    let due = t0 + WINDOW_ESTIMATED_SECS;
    let mut tracker = DisputeTracker::default();

    tracker.track("est-1", true, t0);
    // 窗口内不可终局入账。
    assert!(matches!(tracker.finalize("est-1", due - 1), Err(Error::DisputeWindow(_))));
    // 窗口内争议成功；争议未决时届满也不得终局，重复争议同样被拒。
    assert!(tracker.dispute("est-1", due - 1).is_ok());
    assert!(matches!(tracker.finalize("est-1", due + 1), Err(Error::DisputeWindow(_))));
    assert!(matches!(tracker.dispute("est-1", due), Err(Error::DisputeWindow(_))));

    // 无争议：窗口届满可终局；迟到争议被拒，终局后争议同样被拒。
    tracker.track("est-2", true, t0);
    assert!(matches!(tracker.dispute("est-2", due + 1), Err(Error::DisputeWindow(_))));
    assert!(tracker.finalize("est-2", due + 1).is_ok());
    assert!(matches!(tracker.dispute("est-2", due + 2), Err(Error::DisputeWindow(_))));

    // 普通收据走 24h 档（Q3）：届满前不可终局，届满后可。
    tracker.track("ok-1", false, t0);
    assert!(matches!(tracker.finalize("ok-1", t0 + WINDOW_SECS - 1), Err(Error::DisputeWindow(_))));
    assert!(tracker.finalize("ok-1", t0 + WINDOW_SECS + 1).is_ok());
}

#[test]
fn dispute_unknown_req_rejected() {
    let mut tracker = DisputeTracker::default();
    assert!(matches!(tracker.dispute("ghost", 0), Err(Error::NotFound(_))));
    assert!(matches!(tracker.finalize("ghost", 0), Err(Error::NotFound(_))));
}
