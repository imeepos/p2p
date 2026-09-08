//! B 波链接侧用例：B1 双协议 roundtrip（claude usage 合成命中）/ B4 链接全链
//! （create→redeem→allowlist source=share:<id>→borrow→bound-other→revoke 级联
//! →expired）/ B5 泄露面（P2P 帧与上游 HTTP 体不含 apiKey；台账/列表无 token 原文）。

mod llm_share_common;

#[path = "llm_share_b_common/fx.rs"]
mod fx;
#[path = "llm_share_b_common/http_mock.rs"]
mod http_mock;
#[path = "llm_share_b_common/serve.rs"]
mod serve;
#[path = "llm_share_b_common/tap.rs"]
mod tap;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use fx::{expect_finished, ShareFx, KEY, MODEL};
use http_mock::MockHttpUpstream;
use llm_share_common::{
    call, proxy_request, rig_custom, route, sse_data, usage_chunk, MockUpstream, Script,
};
use llm_share_link::link::build_link;
use llm_share_link::redeem::RedeemCode;
use llm_share_link::redeem::RedeemResponse;
use llm_share_link::token::generate_token;
use llm_share_proxy::upstream::Upstream;
use llm_share_proxy::upstream_http::HttpUpstream;
use llm_share_proxy::{ClaudeUpstream, ErrorCode, ModelRoute, ProxyEvent};
use p2p_cli::llm_share::allowlist;
use p2p_cli::llm_share::share::{share_list, share_revoke};
use tap::WireLog;

type RouteFactory = Arc<dyn Fn() -> HashMap<String, ModelRoute> + Send + Sync>;

fn http_route(base: String, upstream: Arc<dyn Upstream>) -> ModelRoute {
    ModelRoute {
        base_url: base,
        api_key: KEY.to_owned(),
        upstream,
    }
}

fn mock_routes(mock: Arc<MockUpstream>) -> RouteFactory {
    Arc::new(move || HashMap::from([(MODEL.to_owned(), route(mock.clone()))]))
}

fn openai_models(mock: &MockHttpUpstream) -> HashMap<String, ModelRoute> {
    HashMap::from([(
        MODEL.to_owned(),
        http_route(
            mock.base(),
            Arc::new(HttpUpstream::new().expect("http client")),
        ),
    )])
}

/// B1：openai 直发与 claude 翻译双链路 SSE 语义一致；claude 侧 usage 必须经
/// message_start input_tokens + message_delta output_tokens 合成命中。
#[tokio::test]
async fn b1_dual_protocol_roundtrip_matches_semantics() {
    let openai_frames = [
        sse_data("{\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}"),
        sse_data("{\"choices\":[],\"usage\":{\"prompt_tokens\":11,\"completion_tokens\":30}}"),
        sse_data("[DONE]"),
    ];
    let openai_mock = MockHttpUpstream::start(vec![openai_frames.concat()]).await;
    let claude_frames = [
        sse_data("{\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":7}}}"),
        sse_data("{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}"),
        sse_data("{\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":45}}"),
        sse_data("{\"type\":\"message_stop\"}"),
    ];
    let claude_mock = MockHttpUpstream::start(vec![claude_frames.concat()]).await;
    let models = HashMap::from([
        (
            MODEL.to_owned(),
            http_route(
                openai_mock.base(),
                Arc::new(HttpUpstream::new().expect("http client")),
            ),
        ),
        (
            "claude-sonnet-4".to_owned(),
            http_route(
                claude_mock.base(),
                Arc::new(ClaudeUpstream::new().expect("claude client")),
            ),
        ),
    ]);
    let rig = rig_custom("b1", models, Vec::new(), 1_000_000).await;
    let openai_events = call(&rig, &proxy_request("req-b1-openai", MODEL, 400)).await;
    let receipt = expect_finished(&openai_events, "openai 链");
    assert_eq!(
        (receipt.usage.input, receipt.usage.output),
        (11, 30),
        "openai usage 须与账单一致: events={openai_events:?} recorded={:?}",
        openai_mock.recorded(),
    );
    assert_eq!(receipt.upstream_hint, "openai");
    let claude_events = call(
        &rig,
        &proxy_request("req-b1-claude", "claude-sonnet-4", 400),
    )
    .await;
    let claude_receipt = expect_finished(&claude_events, "claude 链");
    assert_eq!(
        (claude_receipt.usage.input, claude_receipt.usage.output),
        (7, 45),
        "usage 须经 message_start+message_delta 合成命中"
    );
    assert_eq!(claude_receipt.upstream_hint, "claude");
    let sse: Vec<String> = claude_events
        .iter()
        .filter_map(|e| match e {
            ProxyEvent::Sse(d) => Some(d.clone()),
            _ => None,
        })
        .collect();
    assert!(
        sse.iter().any(|d| d.contains("hello")),
        "内容帧语义一致: {sse:?}"
    );
    assert!(
        sse.iter().any(|d| d.contains("finish_reason")),
        "finish 帧语义一致"
    );
    assert_eq!(
        sse.last().map(String::as_str),
        Some("data: [DONE]"),
        "[DONE] 终结"
    );
    let claude_body = &claude_mock.recorded()[0].body;
    assert!(
        claude_body.contains("claude-sonnet-4") && claude_body.contains("messages"),
        "上游收到 Claude 翻译体: {claude_body}"
    );
    assert_eq!(openai_mock.request_count(), 1);
    assert_eq!(claude_mock.request_count(), 1);
}

/// B4：share_create → 链接 → 兑换（allowlist 落 source=share:<id>，模型/到期
/// 正确）→ borrow 成功；异 peer 二次兑换 bound-other 拒。
#[tokio::test]
async fn b4_link_full_chain_redeem_allowlist_borrow() {
    let fx = ShareFx::setup(
        "b4a",
        mock_routes(MockUpstream::new(vec![Script::Canned(vec![usage_chunk(
            10, 5,
        )])])),
    )
    .await;
    assert_eq!(
        fx.link.matches(&fx.token).count(),
        1,
        "token 原文仅在链接出现一次"
    );
    assert_eq!(fx.redeem().await, RedeemResponse::ok());
    let list = allowlist::list(&fx.data_dir).expect("allowlist 可读");
    assert_eq!(list.peers.len(), 1, "兑换恰好落一条 allowlist");
    let entry = &list.peers[0];
    assert_eq!(entry.peer_id, fx.b_peer.to_string(), "绑定首兑换 peer");
    assert_eq!(
        entry.source.as_deref(),
        Some(format!("share:{}", fx.share_id).as_str()),
        "source=share:<id>"
    );
    assert_eq!(entry.models, vec![MODEL.to_owned()], "模型集正确");
    assert_eq!(entry.expires_at, Some(fx.expires_at), "到期正确");
    let events = fx.borrow("req-b4a").await;
    expect_finished(&events, "B");
    let c = fx.intruder("c").await;
    let denied = fx.redeem_with(&c, &fx.link).await;
    assert_eq!(
        denied.code,
        Some(RedeemCode::BoundOther),
        "异 peer 二次兑换被拒"
    );
    assert_eq!(
        allowlist::list(&fx.data_dir).expect("list").peers.len(),
        1,
        "白名单仍只有 B"
    );
    c.shutdown();
}

/// B4 续：revoke 级联移除 allowlist 后借用被动态闸拒、再兑换 share-revoked；
/// 过期条目兑换 expired 拒。
#[tokio::test]
async fn b4_revoke_cascades_and_expired_denied() {
    let fx = ShareFx::setup(
        "b4b",
        mock_routes(MockUpstream::new(vec![Script::Canned(vec![usage_chunk(
            10, 5,
        )])])),
    )
    .await;
    assert_eq!(fx.redeem().await, RedeemResponse::ok());
    let report = share_revoke(&fx.data_dir, &fx.share_id).expect("revoke");
    assert_eq!(report.allowlist_removed, 1, "revoke 级联移除 allowlist");
    assert!(allowlist::list(&fx.data_dir)
        .expect("list")
        .peers
        .is_empty());
    let events = fx.borrow("req-b4b-revoked").await;
    assert!(
        matches!(
            events.last(),
            Some(ProxyEvent::Rejected {
                code: ErrorCode::NotAllowlisted,
                ..
            })
        ),
        "borrow 须被动态闸拒: {events:?}"
    );
    assert_eq!(
        fx.redeem().await.code,
        Some(RedeemCode::ShareRevoked),
        "撤销后再兑换被拒"
    );
    let token = generate_token();
    let expired_link = fx.expired_share_link(&token);
    let denied = fx.redeem_with(&fx.b, &expired_link).await;
    assert_eq!(denied.code, Some(RedeemCode::Expired), "过期兑换被拒");
}

/// B5：P2P 帧（双向）与上游 mock 收到的 HTTP 体不含 apiKey 明文（仅允许出现
/// 在对上游鉴权头）；share 台账/列表无 token 原文，token 原文仅在链接一次。
#[tokio::test]
async fn b5_key_and_token_never_leak() {
    let mock = MockHttpUpstream::start(vec![usage_chunk(10, 5)]).await;
    let rig = rig_custom("b5", openai_models(&mock), Vec::new(), 1_000_000).await;
    // 数据目录 + 换装 tap 包装的 serve（registry insert 语义覆盖原 handler）
    let root = std::env::temp_dir().join(format!("llm-e2e-b5-data-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let data_dir = root.join("llm").display().to_string();
    std::fs::create_dir_all(&data_dir).expect("b5 data dir");
    allowlist::allow(
        &data_dir,
        &rig.b_peer.to_string(),
        &[MODEL.to_owned()],
        None,
        None,
        None,
        "b5",
    )
    .expect("预授权借方 B");
    let wire_log: WireLog = Arc::new(Mutex::new(Vec::new()));
    let mock_for_tap = mock.clone();
    let handler = serve::ShareProxyHandler::new(
        &data_dir,
        Arc::new(move || openai_models(&mock_for_tap)),
        "2026-09",
        1_000_000,
        rig.keypair.clone(),
    );
    rig.a.handle_protocol(Arc::new(tap::TapHandler::new(
        Arc::new(handler),
        wire_log.clone(),
    )));
    let events = call(&rig, &proxy_request("req-b5", MODEL, 400)).await;
    expect_finished(&events, "B");
    let wire = wire_log.lock().expect("wire log").clone();
    let key_bytes = KEY.as_bytes();
    assert!(
        !wire.windows(key_bytes.len()).any(|w| w == key_bytes),
        "P2P 帧双向不含 apiKey 明文"
    );
    let recorded = mock.recorded();
    assert_eq!(recorded.len(), 1);
    assert!(!recorded[0].body.contains(KEY), "上游 HTTP 体不含 apiKey");
    assert_eq!(
        recorded[0].authorization.as_deref(),
        Some(format!("Bearer {KEY}").as_str()),
        "apiKey 仅在鉴权头"
    );
    assert!(recorded[0].x_api_key.is_none());
    // share 台账/列表无 token 原文；链接含 token 恰一次
    let token = generate_token();
    let now = p2p_cli::llm_share::now_secs();
    let mut ledger = llm_share_link::ledger::ShareLedger::new();
    let share_id = ledger.create(&token, "p1", vec![MODEL.to_owned()], now + 3600, "b5", now);
    let shares = serve::shares_file(&data_dir);
    ledger.save(&shares).expect("save ledger");
    let raw = std::fs::read_to_string(&shares).expect("read shares.json");
    assert!(!raw.contains(&token), "台账无 token 原文");
    let list = share_list(&data_dir, now).expect("share list");
    assert!(
        !serde_json::to_string(&list)
            .expect("encode")
            .contains(&token),
        "share 列表无 token 原文"
    );
    let link = build_link(
        &rig.a_peer.to_string(),
        &[],
        &token,
        None,
        Some(share_id),
        Some(vec![MODEL.to_owned()]),
    );
    assert_eq!(link.matches(&token).count(), 1, "token 原文仅在链接一次");
    let _ = std::fs::remove_dir_all(&root);
}
