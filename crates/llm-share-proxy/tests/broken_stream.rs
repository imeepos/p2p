//! 断流计费（MVP A6）：上游中途断开且无 usage -> 流式估算入账，收据 estimated=true。

mod common;

use common::{proxy_config, proxy_request, sse_data, MockUpstream, Script, TIMEOUT};
use llm_share_proxy::{estimate_tokens, ProxyEvent};
use p2p_identity::Keypair;

#[tokio::test]
async fn broken_stream_bills_by_estimate_with_flagged_receipt() {
    let lender = Keypair::generate();
    let borrower = Keypair::generate();
    let chunk = sse_data("{\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}");
    let mock = MockUpstream::new(vec![Script::BrokenAfter(vec![chunk.clone()])]);
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
    let req = proxy_request("req-broken", "gpt-4o", 400);
    let raw_len = serde_json::to_vec(&req).expect("json").len();
    let events = client
        .call(lender.peer_id(), &req, lender.public(), TIMEOUT)
        .await
        .expect("call");
    assert_eq!(mock.calls(), 1);
    let Some(ProxyEvent::Finished {
        receipt,
        stream_broken: true,
    }) = events.last()
    else {
        panic!("expected broken Finished, got {events:?}");
    };
    assert!(receipt.estimated, "断流收据必须标 estimated=true");
    assert_eq!(
        (receipt.usage.input, receipt.usage.output),
        (estimate_tokens(raw_len), estimate_tokens(chunk.len())),
        "输入按请求线字节、输出按已转发字节估算"
    );
    assert!(
        receipt.verify(&lender.public()).is_ok(),
        "估算收据同样签名生效"
    );
    // 估算流水已入账且解冻（后续同预算请求可走通）。
    let total = receipt.usage.input + receipt.usage.output;
    assert_eq!(
        proxy
            .ledger()
            .await
            .net(&lender.peer_id().to_string(), "2026-09"),
        total as i64
    );
}
