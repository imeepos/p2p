//! borrow 三场景验收（F11/PR6，进程内夹具，不出网）：
//! 1 成功入账 + 收据验签往返（现有 receipt verify / ledger 逻辑直读）；
//! 2 未授权结构化拒绝（NotAllowlisted，上游零调用、流水零产生）；
//! 3 断流 estimated 收据（A6：显式断流信号 + 72h 争议窗 + 验签仍须通过）；
//! 4 req_id 重放幂等（A4：出借方重放回传，借方账本只记一笔）。

mod llm_share_borrow_common;

use std::path::Path;

use llm_share_ledger::{WINDOW_ESTIMATED_SECS, WINDOW_SECS};
use p2p_cli::llm_share::borrow;
use p2p_cli::llm_share::ledger::{self, LedgerFilters};
use p2p_cli::llm_share::receipt;

use llm_share_borrow_common::{canned_ok, content_only, lender_pubkey, params, rig, Script};

#[tokio::test]
async fn borrow_roundtrip_records_and_verifies() {
    let r = rig("ok", vec![Script::Canned(canned_ok())], true).await;
    let report = borrow::run(&params(&r, "req-ok")).await.expect("borrow ok");
    assert_eq!(report.status, "done");
    assert_eq!((report.input, report.output), (21, 9));
    assert!(!report.estimated);
    assert!(report.appended);
    assert_eq!(report.dispute_window_secs, WINDOW_SECS);
    assert_eq!(report.sse_frames, 3);

    // 现有命令面逻辑直读：单笔收据文件离线验签 PASS。
    let verdict = receipt::verify_file(Path::new(&report.receipt_file), &lender_pubkey(&r))
        .expect("verify runs");
    assert_eq!(verdict.verdict, "PASS");
    // ledger list / balance 直接可读（§5.1 wire 形态）。
    let list = ledger::list(&r.ledger_dir_str(), LedgerFilters::default()).expect("list");
    assert_eq!(list.count, 1);
    assert_eq!(list.entries[0].req_id, "req-ok");
    let balance = ledger::balance(&r.ledger_dir_str(), &r.borrower_peer, None).expect("balance");
    assert_eq!(balance.rows.len(), 1);
    assert_eq!(balance.rows[0].net, -30);
    assert_eq!(r.mock.calls(), 1);
}

#[tokio::test]
async fn borrow_unauthorized_rejected_structurally() {
    let r = rig("deny", vec![Script::Canned(canned_ok())], false).await;
    let report = borrow::run(&params(&r, "req-deny")).await.expect("report");
    assert_eq!(report.status, "rejected");
    assert_eq!(report.code.as_deref(), Some("not_allowlisted"));
    assert!(report
        .message
        .as_deref()
        .is_some_and(|m| m.contains("not in allowlist")));
    assert_eq!(report.input + report.output, 0);
    assert!(!report.appended);
    assert!(!report.estimated);
    // 预检拒绝：上游零调用、流水零产生（ledger 文件不入盘）。
    assert_eq!(r.mock.calls(), 0);
    assert!(!r.ledger_path().exists());
}

#[tokio::test]
async fn borrow_broken_stream_estimated_receipt() {
    let r = rig("broken", vec![Script::BrokenAfter(content_only())], true).await;
    let report = borrow::run(&params(&r, "req-broken"))
        .await
        .expect("report");
    assert_eq!(report.status, "stream_broken");
    assert!(report.estimated, "断流无 usage 必须出 estimated 收据");
    assert_eq!(report.dispute_window_secs, WINDOW_ESTIMATED_SECS);
    assert!(report.appended);
    // 断流路径验签仍须通过（落盘前复验 + 离线 verify 双重断言）。
    let verdict = receipt::verify_file(Path::new(&report.receipt_file), &lender_pubkey(&r))
        .expect("verify runs");
    assert_eq!(verdict.verdict, "PASS");
    let list = ledger::list(&r.ledger_dir_str(), LedgerFilters::default()).expect("list");
    assert_eq!(list.count, 1);
    assert!(list.entries[0].estimated);
    assert_eq!(r.mock.calls(), 1);
}

#[tokio::test]
async fn borrow_replayed_req_id_records_once() {
    let r = rig(
        "dup",
        vec![Script::Canned(canned_ok()), Script::Canned(canned_ok())],
        true,
    )
    .await;
    let first = borrow::run(&params(&r, "req-dup")).await.expect("first");
    assert_eq!(first.status, "done");
    assert!(first.appended);
    // 同 req_id 重放：出借方回传原收据（duplicate_req_id），借方账本不双记。
    let replay = borrow::run(&params(&r, "req-dup")).await.expect("replay");
    assert_eq!(replay.status, "rejected");
    assert_eq!(replay.code.as_deref(), Some("duplicate_req_id"));
    assert!(!replay.appended);
    let list = ledger::list(&r.ledger_dir_str(), LedgerFilters::default()).expect("list");
    assert_eq!(list.count, 1);
    assert_eq!(r.mock.calls(), 1, "重放路径上游零调用");
}
