//! 正常链路（MVP A1/A3/A4）：SSE roundtrip、usage 提取、收据验签/篡改必败、
//! 双边流水一致、req_id 重放单笔。

mod common;

use common::{proxy_config, proxy_request, sse_data, usage_chunk, MockUpstream, Script, TIMEOUT};
use llm_share_ledger::Ledger;
use llm_share_proxy::{ErrorCode, ProxyEvent};
use p2p_identity::Keypair;

#[tokio::test]
async fn sse_roundtrip_usage_settlement_and_ledger_consistency() {
    let lender = Keypair::generate();
    let borrower = Keypair::generate();
    let mock = MockUpstream::new(vec![Script::Canned(vec![
        sse_data("{\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}"),
        usage_chunk(123, 45),
        sse_data("[DONE]"),
    ])]);
    let (proxy, client) = common::spin(
        &lender,
        proxy_config(
            &lender.peer_id().to_string(),
            &[&borrower],
            mock.clone(),
            1_000_000,
            4,
        ),
        borrower.peer_id(),
    )
    .await;
    let req = proxy_request("req-ok", "gpt-4o", 400);
    let events = client
        .call(lender.peer_id(), &req, lender.public(), TIMEOUT)
        .await
        .expect("call");
    assert_eq!(mock.calls(), 1);
    let sse: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            ProxyEvent::Sse(d) => Some(d.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(sse.len(), 3, "SSE 事件须逐帧转发: {sse:?}");
    assert!(sse[0].starts_with("data: "));
    let Some(ProxyEvent::Finished {
        receipt,
        stream_broken: false,
    }) = events.last()
    else {
        panic!("expected Finished, got {events:?}");
    };
    assert_eq!(
        (receipt.usage.input, receipt.usage.output),
        (123, 45),
        "usage 从流末提取"
    );
    assert!(!receipt.estimated);
    assert!(receipt.verify(&lender.public()).is_ok(), "收据验签必须通过");
    // 篡改必败（MVP A3）。
    let mut tampered = receipt.clone();
    tampered.usage.output += 1;
    assert!(
        tampered.verify(&lender.public()).is_err(),
        "篡改 usage 后验签必须失败"
    );
    // 双边流水一致：出借方视角 +168，借方视角 -168；镜像入借方账本可对账。
    let total = receipt.usage.input + receipt.usage.output;
    let lender_ledger = proxy.ledger().await;
    assert_eq!(
        lender_ledger.net(&lender.peer_id().to_string(), "2026-09"),
        total as i64
    );
    assert_eq!(
        lender_ledger.net(&borrower.peer_id().to_string(), "2026-09"),
        -(total as i64)
    );
    let mut borrower_ledger = Ledger::default();
    assert!(borrower_ledger
        .apply(receipt, &lender.public())
        .expect("apply"));
    let recon = lender_ledger.reconcile(&borrower_ledger);
    assert_eq!(
        (
            recon.matched,
            recon.local_only.len(),
            recon.remote_only.len()
        ),
        (1, 0, 0)
    );
}

#[tokio::test]
async fn replay_req_id_billed_once_and_returns_original_receipt() {
    let lender = Keypair::generate();
    let borrower = Keypair::generate();
    let mock = MockUpstream::new(vec![Script::Canned(vec![usage_chunk(10, 5)])]);
    let (proxy, client) = common::spin(
        &lender,
        proxy_config(
            &lender.peer_id().to_string(),
            &[&borrower],
            mock.clone(),
            1_000_000,
            4,
        ),
        borrower.peer_id(),
    )
    .await;
    let req = proxy_request("req-replay", "gpt-4o", 400);
    let first = client
        .call(lender.peer_id(), &req, lender.public(), TIMEOUT)
        .await
        .expect("first call");
    let Some(ProxyEvent::Finished { receipt, .. }) = first.last() else {
        panic!("expected Finished, got {first:?}");
    };
    assert_eq!(
        proxy
            .ledger()
            .await
            .net(&lender.peer_id().to_string(), "2026-09"),
        15,
        "实际 usage 入账"
    );
    let second = client
        .call(lender.peer_id(), &req, lender.public(), TIMEOUT)
        .await
        .expect("replay call");
    assert!(matches!(
        second.last(),
        Some(ProxyEvent::Rejected {
            code: ErrorCode::DuplicateReqId,
            receipt: Some(_),
            ..
        })
    ));
    let Some(ProxyEvent::Rejected {
        receipt: Some(replayed),
        ..
    }) = second.last()
    else {
        panic!("unreachable");
    };
    assert_eq!(replayed, receipt, "重放回传同次收据");
    assert_eq!(mock.calls(), 1, "重放不得再打上游");
    assert_eq!(proxy.receipts().await.len(), 1, "同 req_id 只记一笔");
    assert_eq!(
        proxy
            .ledger()
            .await
            .net(&borrower.peer_id().to_string(), "2026-09"),
        -15
    );
}

/// usage 超冻结额（上游超发，病理场景）：结算显式失败、零流水，不静默吞账。
#[tokio::test]
async fn usage_over_estimate_fails_settle_without_entry() {
    let lender = Keypair::generate();
    let borrower = Keypair::generate();
    let mock = MockUpstream::new(vec![Script::Canned(vec![usage_chunk(100_000, 200_000)])]);
    let (proxy, client) = common::spin(
        &lender,
        proxy_config(
            &lender.peer_id().to_string(),
            &[&borrower],
            mock.clone(),
            1_000_000,
            4,
        ),
        borrower.peer_id(),
    )
    .await;
    let req = proxy_request("req-over", "gpt-4o", 400);
    let events = client
        .call(lender.peer_id(), &req, lender.public(), TIMEOUT)
        .await
        .expect("call");
    assert!(matches!(
        events.last(),
        Some(ProxyEvent::Rejected {
            code: ErrorCode::SettleFailed,
            ..
        })
    ));
    assert!(proxy.receipts().await.is_empty(), "结算失败不产生流水");
    assert_eq!(
        proxy
            .ledger()
            .await
            .net(&lender.peer_id().to_string(), "2026-09"),
        0
    );
}
