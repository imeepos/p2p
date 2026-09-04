//! E10-T20 双节点 E2E（idle-token-sharing-plan §10 验收口径 A1-A7）：
//! 真 facade Node TCP 互联 + 进程内 mock 上游，覆盖
//! A1 流式问答语义一致 / A2 冻结不足结构化拒绝零上游 / A3 双边流水一致+收据验签+篡改必败 /
//! A4 req_id 重放单笔 / A5 非 allowlist 拒+声明 TTL 过期不选路 / A6 断流 estimated 收据+争议窗口。
//! A7 = 三 crate 测试 + workspace make check，由验收命令机械判定，不落本文件。

mod llm_share_common;

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use llm_share_common::{
    call, client_for, extra_node, proxy_request, rig, sse_data, usage_chunk, MockUpstream, Script,
    STEP,
};
use llm_share_ledger::{DisputeTracker, Ledger};
use llm_share_offer::{select_offers, Offer, OfferBook, RateLimit, SignedOffer, VerifyError};
use llm_share_proxy::{ErrorCode, ProxyEvent};

/// A1：B 以 OpenAI chat completions 请求经真链路代理获得流式回答，
/// SSE 逐帧语义与直连 mock 上游一致，usage 与上游账单一致。
#[tokio::test]
async fn a1_streaming_roundtrip_matches_direct_upstream() {
    let payloads = [
        "{\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}",
        "{\"usage\":{\"prompt_tokens\":123,\"completion_tokens\":45}}",
        "[DONE]",
    ];
    let mock = MockUpstream::new(vec![Script::Canned(
        payloads.iter().map(|d| sse_data(d)).collect(),
    )]);
    let rig = rig("a1", mock.clone(), 1_000_000).await;
    let events = call(&rig, &proxy_request("req-a1", "gpt-4o", 400)).await;
    assert_eq!(mock.calls(), 1, "上游恰调用一次");
    let sse: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            ProxyEvent::Sse(d) => Some(d.clone()),
            _ => None,
        })
        .collect();
    let want: Vec<_> = payloads.iter().map(|d| format!("data: {d}")).collect();
    assert_eq!(sse, want, "SSE 逐帧透传，语义与直连上游一致: {sse:?}");
    let Some(ProxyEvent::Finished {
        receipt,
        stream_broken: false,
    }) = events.last()
    else {
        panic!("须以 Done 终结: {events:?}");
    };
    assert_eq!((receipt.usage.input, receipt.usage.output), (123, 45));
    assert!(
        receipt.verify(&rig.keypair.public()).is_ok(),
        "收据验签通过"
    );
}

/// A2：冻结不足（净差超限）结构化拒绝，上游零调用、双边零流水。
#[tokio::test]
async fn a2_freeze_insufficient_rejected_zero_upstream_calls() {
    let mock = MockUpstream::new(vec![Script::Canned(vec![usage_chunk(1, 1)])]);
    // net_limit=1：est = 输入估算 + max_tokens(400) 必然超限，冻结硬闸拒绝
    let rig = rig("a2", mock.clone(), 1).await;
    let events = call(&rig, &proxy_request("req-a2", "gpt-4o", 400)).await;
    let Some(ProxyEvent::Rejected {
        code: ErrorCode::FreezeInsufficient,
        message,
        ..
    }) = events.last()
    else {
        panic!("须结构化冻结拒绝: {events:?}");
    };
    assert!(!message.is_empty(), "拒绝须可述原因");
    assert_eq!(mock.calls(), 0, "拒绝路径上游零调用");
    let ledger = rig.proxy.ledger().await;
    assert_eq!(ledger.net(&rig.a_peer.to_string(), "2026-09"), 0, "零流水");
    assert_eq!(ledger.net(&rig.b_peer.to_string(), "2026-09"), 0, "零流水");
}

/// A3：一笔完整调用后双边账本流水一致，B 镜像入账可对账；
/// 收据验签通过，篡改 usage 后验签必败。
#[tokio::test]
async fn a3_dual_ledger_consistency_receipt_sig_and_tamper() {
    let mock = MockUpstream::new(vec![Script::Canned(vec![usage_chunk(100, 50)])]);
    let rig = rig("a3", mock.clone(), 1_000_000).await;
    let events = call(&rig, &proxy_request("req-a3", "gpt-4o", 400)).await;
    let Some(ProxyEvent::Finished { receipt, .. }) = events.last() else {
        panic!("须正常终结: {events:?}");
    };
    assert_eq!(receipt.lender, rig.a_peer.to_string(), "流水归属出借方");
    assert_eq!(receipt.borrower, rig.b_peer.to_string(), "流水归属借方");
    assert!(receipt.verify(&rig.keypair.public()).is_ok());
    let mut tampered = receipt.clone();
    tampered.usage.output += 1;
    assert!(tampered.verify(&rig.keypair.public()).is_err(), "篡改必败");
    let total = (receipt.usage.input + receipt.usage.output) as i64;
    let a_ledger = rig.proxy.ledger().await;
    assert_eq!(a_ledger.net(&rig.a_peer.to_string(), "2026-09"), total);
    assert_eq!(a_ledger.net(&rig.b_peer.to_string(), "2026-09"), -total);
    let mut b_ledger = Ledger::default();
    assert!(b_ledger
        .apply(receipt, &rig.keypair.public())
        .expect("apply"));
    let recon = a_ledger.reconcile(&b_ledger);
    assert_eq!(
        (
            recon.matched,
            recon.local_only.len(),
            recon.remote_only.len()
        ),
        (1, 0, 0),
        "双边哈希链一致"
    );
}

/// A4：同 req_id 重放仅一笔流水，重放得到结构化拒绝并回传原收据，上游零增量。
#[tokio::test]
async fn a4_replay_req_id_billed_once() {
    let mock = MockUpstream::new(vec![Script::Canned(vec![usage_chunk(10, 5)])]);
    let rig = rig("a4", mock.clone(), 1_000_000).await;
    let req = proxy_request("req-a4", "gpt-4o", 400);
    let first = call(&rig, &req).await;
    let Some(ProxyEvent::Finished { receipt, .. }) = first.last() else {
        panic!("首笔须终结: {first:?}");
    };
    let second = call(&rig, &req).await;
    let Some(ProxyEvent::Rejected {
        code: ErrorCode::DuplicateReqId,
        receipt: Some(replayed),
        ..
    }) = second.last()
    else {
        panic!("重放须结构化拒绝并回传原收据: {second:?}");
    };
    assert_eq!(replayed, receipt, "重放回传同次收据");
    assert_eq!(mock.calls(), 1, "重放不得再打上游");
    assert_eq!(rig.proxy.receipts().await.len(), 1, "同 req_id 仅一笔");
    let ledger = rig.proxy.ledger().await;
    assert_eq!(ledger.net(&rig.a_peer.to_string(), "2026-09"), 15);
    assert_eq!(ledger.net(&rig.b_peer.to_string(), "2026-09"), -15);
}

/// A5：非 allowlist PeerId 真实入站被三闸首闸拒（上游零增量、零流水）；
/// 能力声明 TTL 过期后不再被选路（OfferBook 失效 + 选路器剔除 + 信封过期可验）。
#[tokio::test]
async fn a5_non_allowlist_rejected_and_expired_offer_not_routed() {
    let mock = MockUpstream::new(vec![Script::Canned(vec![usage_chunk(10, 5)])]);
    let rig = rig("a5", mock.clone(), 1_000_000).await;
    let ok = call(&rig, &proxy_request("req-a5-b", "gpt-4o", 400)).await;
    assert!(
        matches!(ok.last(), Some(ProxyEvent::Finished { .. })),
        "{ok:?}"
    );
    let c = extra_node("a5", "c", &rig).await;
    let c_client = client_for(c.clone());
    let events = c_client
        .call(
            rig.a_peer,
            &proxy_request("req-a5-c", "gpt-4o", 400),
            rig.keypair.public(),
            STEP,
        )
        .await
        .expect("C 拨号");
    assert!(
        matches!(
            events.last(),
            Some(ProxyEvent::Rejected {
                code: ErrorCode::NotAllowlisted,
                ..
            })
        ),
        "非白名单须拒: {events:?}"
    );
    assert_eq!(mock.calls(), 1, "拒入站不触上游");
    assert_eq!(
        rig.proxy
            .ledger()
            .await
            .net(&rig.a_peer.to_string(), "2026-09"),
        15,
        "C 请求零流水"
    );
    c.shutdown();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let offer = Offer {
        peer: rig.a_peer.to_string(),
        models: vec!["gpt-4o".to_string()],
        spare: BTreeMap::from([("gpt-4o".to_string(), 1_000_000)]),
        period_ends: "2026-09-30".to_string(),
        max_per_req: BTreeMap::new(),
        rate_limit: RateLimit {
            rpm: 60,
            concurrency: 2,
        },
        ttl_secs: 1,
        retention: "none".to_string(),
    };
    let signed = SignedOffer::sign(&offer, &rig.keypair, now).expect("sign offer");
    let mut book = OfferBook::new();
    book.insert(signed.clone(), now).expect("insert offer");
    assert_eq!(
        select_offers(book.live(now), "gpt-4o", now).len(),
        1,
        "有效期内被选路"
    );
    let later = now + offer.ttl_secs + 1;
    assert!(book.live(later).is_empty(), "TTL 过期即失效出簿");
    assert!(
        select_offers(book.live(later), "gpt-4o", later).is_empty(),
        "过期不再被选路"
    );
    assert!(
        matches!(signed.verify(later), Err(VerifyError::Expired(_))),
        "信封过期可验"
    );
}

/// A6：上游断流产生 estimated=true 估算收据并如实入账；争议窗口接口可用，
/// 争议未决不得终局。
#[tokio::test]
async fn a6_broken_stream_estimated_receipt_and_dispute() {
    let mock = MockUpstream::new(vec![Script::BrokenAfter(vec![sse_data(
        "{\"choices\":[{\"delta\":{\"content\":\"par\"}}]}",
    )])]);
    let rig = rig("a6", mock.clone(), 1_000_000).await;
    let events = call(&rig, &proxy_request("req-a6", "gpt-4o", 400)).await;
    let Some(ProxyEvent::Finished {
        receipt,
        stream_broken: true,
    }) = events.last()
    else {
        panic!("断流须以 stream_broken 终结: {events:?}");
    };
    assert!(receipt.estimated, "断流无 usage 须按估算计费");
    assert!(
        events.iter().any(|e| matches!(e, ProxyEvent::Sse(_))),
        "断流前已转发帧须保留"
    );
    let total = (receipt.usage.input + receipt.usage.output) as i64;
    assert_eq!(
        rig.proxy
            .ledger()
            .await
            .net(&rig.b_peer.to_string(), "2026-09"),
        -total,
        "估算入账"
    );
    let mut tracker = DisputeTracker::default();
    tracker.track(&receipt.req_id, receipt.estimated, receipt.ts);
    tracker
        .dispute(&receipt.req_id, receipt.ts + 1)
        .expect("窗口内争议可用");
    assert!(
        tracker.finalize(&receipt.req_id, receipt.ts + 1).is_err(),
        "争议未决不得终局"
    );
}
