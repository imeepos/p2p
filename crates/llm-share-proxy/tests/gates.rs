//! 三闸拒绝路径（MVP A2/A4/A5）：各闸错误码可区分，且 mock 上游调用计数为零。

mod common;

use std::time::Duration;

use common::{proxy_config, proxy_request, until, MockUpstream, Script, TIMEOUT};
use llm_share_proxy::{ErrorCode, ProxyEvent};
use p2p_identity::Keypair;

#[tokio::test]
async fn not_allowlisted_rejected_with_zero_upstream_calls() {
    let lender = Keypair::generate();
    let borrower = Keypair::generate();
    let outsider = Keypair::generate();
    let mock = MockUpstream::new(vec![]);
    let (proxy, client) = common::spin(
        &lender,
        proxy_config(
            &lender.peer_id().to_string(),
            &[&borrower],
            mock.clone(),
            1_000_000,
            4,
        ),
        outsider.peer_id(),
    )
    .await;
    let req = proxy_request("req-allow", "gpt-4o", 64);
    let events = client
        .call(lender.peer_id(), &req, lender.public(), TIMEOUT)
        .await
        .expect("call");
    assert!(
        matches!(
            events.last(),
            Some(ProxyEvent::Rejected {
                code: ErrorCode::NotAllowlisted,
                ..
            })
        ),
        "unexpected events: {events:?}"
    );
    assert_eq!(mock.calls(), 0, "上游必须零调用");
    assert!(proxy.receipts().await.is_empty(), "拒绝路径不得产生流水");
}

#[tokio::test]
async fn model_not_served_rejected_with_zero_upstream_calls() {
    let lender = Keypair::generate();
    let borrower = Keypair::generate();
    let mock = MockUpstream::new(vec![]);
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
    let req = proxy_request("req-model", "claude-3", 64);
    let events = client
        .call(lender.peer_id(), &req, lender.public(), TIMEOUT)
        .await
        .expect("call");
    assert!(matches!(
        events.last(),
        Some(ProxyEvent::Rejected {
            code: ErrorCode::ModelNotServed,
            ..
        })
    ));
    assert_eq!(mock.calls(), 0);
    assert!(proxy.receipts().await.is_empty());
}

#[tokio::test]
async fn freeze_insufficient_rejected_with_zero_upstream_calls() {
    let lender = Keypair::generate();
    let borrower = Keypair::generate();
    let mock = MockUpstream::new(vec![]);
    let (proxy, client) = common::spin(
        &lender,
        proxy_config(
            &lender.peer_id().to_string(),
            &[&borrower],
            mock.clone(),
            100,
            4,
        ),
        borrower.peer_id(),
    )
    .await;
    let req = proxy_request("req-freeze", "gpt-4o", 100_000);
    let events = client
        .call(lender.peer_id(), &req, lender.public(), TIMEOUT)
        .await
        .expect("call");
    assert!(matches!(
        events.last(),
        Some(ProxyEvent::Rejected {
            code: ErrorCode::FreezeInsufficient,
            ..
        })
    ));
    assert_eq!(mock.calls(), 0);
    assert!(proxy.receipts().await.is_empty());
}

#[tokio::test]
async fn concurrency_exceeded_rejected_while_inflight() {
    let lender = Keypair::generate();
    let borrower = Keypair::generate();
    let mock = MockUpstream::new(vec![Script::Stalled]);
    let (proxy, client) = common::spin(
        &lender,
        proxy_config(
            &lender.peer_id().to_string(),
            &[&borrower],
            mock.clone(),
            1_000_000,
            1,
        ),
        borrower.peer_id(),
    )
    .await;
    let (lender_peer, lender_pubkey) = (lender.peer_id(), lender.public());
    let inflight_req = proxy_request("req-c1", "gpt-4o", 64);
    let first = tokio::spawn({
        let client = client.clone();
        async move {
            client
                .call(lender_peer, &inflight_req, lender_pubkey, TIMEOUT)
                .await
        }
    });
    until(|| mock.calls() == 1).await;
    let second = client
        .call(
            lender.peer_id(),
            &proxy_request("req-c2", "gpt-4o", 64),
            lender.public(),
            Duration::from_secs(5),
        )
        .await
        .expect("call");
    assert!(matches!(
        second.last(),
        Some(ProxyEvent::Rejected {
            code: ErrorCode::ConcurrencyExceeded,
            ..
        })
    ));
    assert_eq!(mock.calls(), 1, "超限请求不得打到上游");
    assert!(proxy.receipts().await.is_empty());
    first.abort();
}

#[tokio::test]
async fn upstream_rejection_produces_no_entry_and_releases_hold() {
    let lender = Keypair::generate();
    let borrower = Keypair::generate();
    let mock = MockUpstream::new(vec![
        Script::Canned(vec![common::usage_chunk(10, 5)]),
        Script::Rejected(429),
    ]);
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
    let req = proxy_request("req-429", "gpt-4o", 64);
    let events = client
        .call(lender.peer_id(), &req, lender.public(), TIMEOUT)
        .await
        .expect("call");
    assert!(matches!(
        events.last(),
        Some(ProxyEvent::Rejected {
            code: ErrorCode::UpstreamRejected,
            ..
        })
    ));
    assert!(proxy.receipts().await.is_empty(), "上游拒绝不产生流水");
    // 冻结已随 abort 解除：同 req_id 立即重试可完整走通（只弹一个 Script，且为重试准备的 Canned）。
    let retry = client
        .call(lender.peer_id(), &req, lender.public(), TIMEOUT)
        .await
        .expect("retry");
    assert!(matches!(
        retry.last(),
        Some(ProxyEvent::Finished {
            stream_broken: false,
            ..
        })
    ));
    assert_eq!(mock.calls(), 2);
    assert_eq!(proxy.receipts().await.len(), 1);
}
