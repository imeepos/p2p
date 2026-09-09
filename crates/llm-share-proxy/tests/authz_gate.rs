//! 闸 1 判定源切换对照测试（authz-role-design §8/§9，A2）：装配 AuthzChecker 后
//! 有绑定（ally）→放行、无绑定→拒且 wire 码与旧 allowlist 路径同码；
//! allowlist 表装配后不再消费（转只读归档）；闸 2 与未装配回滚路径不回归。

mod common;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use common::{proxy_request, MockUpstream, Script, TIMEOUT};
use llm_share_proxy::{
    AuthzChecker, ErrorCode, Gate1Fn, LenderProxy, ModelRoute, ProxyClient, ProxyConfig, ProxyEvent,
};
use p2p_authz::Authz;
use p2p_identity::{Keypair, PeerId};
use p2p_protocol::LoopbackHub;

/// 与 common::spin 同构，但可装配 authz 闸 1 判定源（None = 回滚路径对照）。
async fn spin_with_gate1(
    lender: &Keypair,
    cfg: ProxyConfig,
    gate1: Option<Gate1Fn>,
    borrower: PeerId,
) -> (Arc<LenderProxy>, ProxyClient<LoopbackHub>) {
    let (hub, mut inbound) = LoopbackHub::new(64, 256 * 1024);
    let mut proxy = LenderProxy::new(cfg, lender.clone());
    if let Some(gate1) = gate1 {
        proxy = proxy.with_gate1_authz(gate1);
    }
    let proxy = Arc::new(proxy);
    let worker = proxy.clone();
    tokio::spawn(async move {
        while let Some(stream) = inbound.recv().await {
            let worker = worker.clone();
            tokio::spawn(async move {
                if let Err(e) = worker.serve(stream, borrower).await {
                    eprintln!("serve error: {e}");
                }
            });
        }
    });
    (proxy, ProxyClient::new(hub))
}

/// 配置：allowlist 默认空表（判定源切换后旧表不参与准入的最强对照）。
fn authz_config(lender_id: &str, upstream: Arc<MockUpstream>) -> ProxyConfig {
    let mut models = HashMap::new();
    models.insert(
        "gpt-4o".to_string(),
        ModelRoute {
            base_url: "https://upstream.invalid/v1".into(),
            api_key: "sk-test".into(),
            upstream,
        },
    );
    ProxyConfig {
        lender_id: lender_id.to_string(),
        period: "2026-09".into(),
        net_limit: 1_000_000,
        max_concurrent: 4,
        allowlist: HashSet::new(),
        models,
    }
}

fn gate1_for(tag: &str, grants: &[(&Keypair, &str, Option<u64>)]) -> Gate1Fn {
    let dir = std::env::temp_dir().join(format!(
        "llm-share-proxy-authz-gate-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    for (key, role, expires_at) in grants {
        Authz::new(&dir, p2p_authz::SystemClock)
            .bind(
                &key.peer_id().to_string(),
                role,
                *expires_at,
                "authz-gate-test",
            )
            .expect("bind");
    }
    AuthzChecker::new(&dir, p2p_authz::SystemClock).into_gate()
}

fn canned_upstream() -> Arc<MockUpstream> {
    MockUpstream::new(vec![Script::Canned(vec![
        common::sse_data("{\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}"),
        common::usage_chunk(10, 5),
        common::sse_data("[DONE]"),
    ])])
}

#[tokio::test]
async fn bound_ally_passes_gate1_and_settles() {
    let lender = Keypair::generate();
    let borrower = Keypair::generate();
    let mock = canned_upstream();
    let gate1 = gate1_for("allow", &[(&borrower, "ally", None)]);
    let (proxy, client) = spin_with_gate1(
        &lender,
        authz_config(&lender.peer_id().to_string(), mock.clone()),
        Some(gate1),
        borrower.peer_id(),
    )
    .await;
    let req = proxy_request("req-authz-allow", "gpt-4o", 400);
    let events = client
        .call(lender.peer_id(), &req, lender.public(), TIMEOUT)
        .await
        .expect("call");
    assert!(
        matches!(events.last(), Some(ProxyEvent::Finished { .. })),
        "绑定 ally 必须放行并结算，实际: {events:?}"
    );
    assert_eq!(mock.calls(), 1, "放行后上游恰好一次调用");
    assert!(!proxy.receipts().await.is_empty(), "结算须产生收据");
}

#[tokio::test]
async fn unbound_borrower_rejected_with_legacy_wire_code() {
    let lender = Keypair::generate();
    let borrower = Keypair::generate();
    let mock = MockUpstream::new(vec![]);
    let gate1 = gate1_for("unbound", &[]);
    // 对照关键点：allowlist 故意含借方——装配 authz 后旧表不得再放行。
    let mut cfg = authz_config(&lender.peer_id().to_string(), mock.clone());
    cfg.allowlist.insert(borrower.peer_id().to_string());
    let (proxy, client) = spin_with_gate1(&lender, cfg, Some(gate1), borrower.peer_id()).await;
    let req = proxy_request("req-authz-deny", "gpt-4o", 64);
    let events = client
        .call(lender.peer_id(), &req, lender.public(), TIMEOUT)
        .await
        .expect("call");
    let Some(ProxyEvent::Rejected { code, message, .. }) = events.last() else {
        panic!("无绑定必须拒绝，实际: {events:?}");
    };
    assert_eq!(code, &ErrorCode::NotAllowlisted, "wire 码必须与旧路径同码");
    assert!(
        message.contains("denied by authz"),
        "判定来源须可归因: {message}"
    );
    assert!(
        message.contains("NotBound"),
        "reason 细化须在 message: {message}"
    );
    assert_eq!(mock.calls(), 0, "上游零调用");
    assert!(proxy.receipts().await.is_empty(), "拒绝路径零流水");
}

#[tokio::test]
async fn wire_code_serialization_identical_to_legacy() {
    assert_eq!(
        serde_json::to_string(&ErrorCode::NotAllowlisted).expect("serializable"),
        "\"not_allowlisted\""
    );
}

#[tokio::test]
async fn expired_binding_denied_with_expired_reason() {
    let lender = Keypair::generate();
    let expired = Keypair::generate();
    let mock = MockUpstream::new(vec![]);
    let gate1 = gate1_for("expired", &[(&expired, "ally", Some(1_000))]);
    let (proxy, client) = spin_with_gate1(
        &lender,
        authz_config(&lender.peer_id().to_string(), mock.clone()),
        Some(gate1),
        expired.peer_id(),
    )
    .await;
    let events = client
        .call(
            lender.peer_id(),
            &proxy_request("req-expired", "gpt-4o", 64),
            lender.public(),
            TIMEOUT,
        )
        .await
        .expect("call");
    let Some(ProxyEvent::Rejected { code, message, .. }) = events.last() else {
        panic!("过期绑定必须拒绝，实际: {events:?}");
    };
    assert_eq!(code, &ErrorCode::NotAllowlisted);
    assert!(message.contains("Expired"), "过期 reason 须细化: {message}");
    assert_eq!(mock.calls(), 0);
    assert!(proxy.receipts().await.is_empty());
}

#[tokio::test]
async fn underprivileged_role_denied_with_missing_perm_reason() {
    let lender = Keypair::generate();
    let friend = Keypair::generate();
    let mock = MockUpstream::new(vec![]);
    let gate1 = gate1_for("perm", &[(&friend, "friend", None)]);
    let (proxy, client) = spin_with_gate1(
        &lender,
        authz_config(&lender.peer_id().to_string(), mock.clone()),
        Some(gate1),
        friend.peer_id(),
    )
    .await;
    let events = client
        .call(
            lender.peer_id(),
            &proxy_request("req-friend", "gpt-4o", 64),
            lender.public(),
            TIMEOUT,
        )
        .await
        .expect("call");
    let Some(ProxyEvent::Rejected { code, message, .. }) = events.last() else {
        panic!("无 llm.borrow 权限必须拒绝，实际: {events:?}");
    };
    assert_eq!(code, &ErrorCode::NotAllowlisted);
    assert!(
        message.contains("MissingPerm"),
        "权限缺失 reason 须细化: {message}"
    );
    assert_eq!(mock.calls(), 0);
    assert!(proxy.receipts().await.is_empty());
}

#[tokio::test]
async fn gate2_model_whitelist_still_enforced_after_gate1() {
    let lender = Keypair::generate();
    let borrower = Keypair::generate();
    let mock = MockUpstream::new(vec![]);
    let gate1 = gate1_for("gate2", &[(&borrower, "ally", None)]);
    let (proxy, client) = spin_with_gate1(
        &lender,
        authz_config(&lender.peer_id().to_string(), mock.clone()),
        Some(gate1),
        borrower.peer_id(),
    )
    .await;
    let req = proxy_request("req-gate2", "claude-3", 64);
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
async fn rollback_path_without_gate1_keeps_legacy_semantics() {
    let lender = Keypair::generate();
    let borrower = Keypair::generate();
    let outsider = Keypair::generate();
    // 放行侧：未装配 gate1 时 allowlist 依旧权威。
    let mock = canned_upstream();
    let mut cfg = authz_config(&lender.peer_id().to_string(), mock.clone());
    cfg.allowlist.insert(borrower.peer_id().to_string());
    let (_proxy, client) = spin_with_gate1(&lender, cfg, None, borrower.peer_id()).await;
    let events = client
        .call(
            lender.peer_id(),
            &proxy_request("req-legacy-allow", "gpt-4o", 400),
            lender.public(),
            TIMEOUT,
        )
        .await
        .expect("call");
    assert!(matches!(events.last(), Some(ProxyEvent::Finished { .. })));
    // 拒绝侧：message 与切换前逐字一致（回滚语义零漂移）。
    let mut cfg = authz_config(&lender.peer_id().to_string(), MockUpstream::new(vec![]));
    cfg.allowlist.insert(borrower.peer_id().to_string());
    let (_, client) = spin_with_gate1(&lender, cfg, None, outsider.peer_id()).await;
    let events = client
        .call(
            lender.peer_id(),
            &proxy_request("req-legacy-deny", "gpt-4o", 64),
            lender.public(),
            TIMEOUT,
        )
        .await
        .expect("call");
    let Some(ProxyEvent::Rejected { message, .. }) = events.last() else {
        panic!("回滚路径外人必须拒绝，实际: {events:?}");
    };
    assert_eq!(
        message.as_str(),
        format!("peer {} not in allowlist", outsider.peer_id()),
        "回滚路径 message 须与切换前逐字一致"
    );
}
