//! serve 装配与门禁测试（W3，PR 轨会签映射：serve_* ↔ §16.6 #3/#4）：
//! gate 判定/过期/跨进程回读、兑换互斥临界区写序、settled 索引重启重建、
//! 模型唯一映射冲突进 lastError、proxy 全链（mock 上游）闸门与收据持久化。

use std::collections::HashMap;
use std::sync::Arc;

use futures::stream::StreamExt;
use llm_share_link::ledger::ShareLedger;
use llm_share_link::redeem::{RedeemCode, RedeemResponse};
use llm_share_proxy::upstream::{SseByteStream, Upstream, UpstreamCall, UpstreamFailure};
use p2p::ProtocolHandler;
use p2p_identity::{load_or_generate_seed, Keypair};
use p2p_protocol::{read_chunked, write_chunked};

use super::common::{cfg_for, peer, seed_identity, store};
use crate::llm_share::serve::gate::AllowlistGate;
use crate::llm_share::serve::map_models_to_providers;
use crate::llm_share::serve::proxy::{ProxyGateHandler, ServeCore};
use crate::llm_share::serve::redeem::RedeemHandler;
use crate::llm_share::LlmShareStore;

fn gate(store: &LlmShareStore) -> AllowlistGate {
    AllowlistGate::load(&store.data_dir()).expect("gate")
}

fn seed_share(store: &LlmShareStore, token: &str) -> String {
    let now = p2p_cli::llm_share::now_secs();
    let mut ledger = ShareLedger::new();
    let share_id = ledger.create(
        token,
        "provider-1",
        vec!["gpt-4o".to_owned()],
        now + 3600,
        "",
        now,
    );
    ledger.save(&store.shares_file()).expect("save shares");
    share_id
}

fn mock_routes() -> HashMap<String, llm_share_proxy::server::ModelRoute> {
    let mut routes = HashMap::new();
    routes.insert(
        "gpt-4o".to_owned(),
        llm_share_proxy::server::ModelRoute {
            base_url: "https://upstream.invalid/v1".to_owned(),
            api_key: "sk-test".to_owned(),
            upstream: Arc::new(MockUpstream) as Arc<dyn Upstream>,
        },
    );
    routes
}

/// serve 记账核心装配助手（测试口径：mock 路由 + 指定 lender 身份与门禁）。
fn serve_core_config(
    store: &LlmShareStore,
    lender_id: String,
    keypair: Keypair,
    gate: Arc<AllowlistGate>,
) -> crate::llm_share::serve::proxy::ServeCoreConfig {
    crate::llm_share::serve::proxy::ServeCoreConfig {
        data_dir: store.data_dir().to_owned(),
        lender_id,
        period: "2026-09-30".to_owned(),
        net_limit: 100_000,
        max_concurrent: 2,
        keypair,
        routes: mock_routes(),
        gate,
    }
}

struct MockUpstream;

type Sse = SseByteStream;

#[async_trait::async_trait]
impl Upstream for MockUpstream {
    async fn chat(&self, _call: UpstreamCall) -> Result<Sse, UpstreamFailure> {
        let sse = "data: {}\n\n".as_bytes().to_vec();
        let usage = "data: {\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":5}}\n\n"
            .as_bytes()
            .to_vec();
        Ok(futures::stream::iter(vec![Ok(sse), Ok(usage)]).boxed())
    }
}
#[test]
fn serve_gate_admits_expiry_and_cross_process_refresh() {
    let (_t, store) = store("serve-gate");
    let g = gate(&store);
    let now = p2p_cli::llm_share::now_secs();
    assert!(!g.admits(&peer(5), now), "默认拒绝：无条目不可用");
    g.allow(
        &peer(5),
        &["gpt-4o".to_owned()],
        None,
        None,
        Some(now + 3600),
        "t0",
    )
    .expect("allow");
    assert!(g.admits(&peer(5), now), "未过期条目放行");
    assert!(!g.admits(&peer(5), now + 3601), "到期经 admit 惰性判定即拒");
    p2p_cli::llm_share::allowlist::allow(&store.data_dir(), &peer(6), &[], None, None, None, "t1")
        .expect("cli allow");
    assert!(g.admits(&peer(6), now), "跨进程写入经 mtime 回读后放行");
}

#[tokio::test]
async fn serve_redeem_critical_section_writes_allowlist_and_binds_peer() {
    let (_t, store) = store("serve-redeem");
    let token = "0123456789abcdef0123456789abcdef";
    let share_id = seed_share(&store, token);
    let g = Arc::new(gate(&store));
    let handler = RedeemHandler::new(store.clone(), g.clone());
    let now = p2p_cli::llm_share::now_secs();
    let ok = handler.activate(&peer(7), token, now).await;
    assert_eq!(ok, RedeemResponse::ok(), "首兑成功");
    let list = p2p_cli::llm_share::allowlist::load_or_empty(&p2p_cli::llm_share::allowlist::path(
        &store.data_dir(),
    ))
    .expect("allowlist");
    let entry = list.entries.get(&peer(7)).expect("兑换落 allowlist 条目");
    assert_eq!(
        entry.source.as_deref(),
        Some(format!("share:{share_id}")).as_deref(),
        "source=share:<shareId>（§16.6 #3）"
    );
    let ledger = ShareLedger::load_or_empty(&store.shares_file()).expect("shares");
    let (_, share) = ledger.iter().next().expect("share entry");
    assert_eq!(share.activations, 1);
    assert_eq!(share.bound_peer.as_deref(), Some(peer(7).as_str()));
    let again = handler.activate(&peer(7), token, now).await;
    assert_eq!(again, RedeemResponse::ok(), "同 peer 幂等成功不计数");
    let other = handler.activate(&peer(8), token, now).await;
    assert_eq!(other, RedeemResponse::deny(RedeemCode::BoundOther));
    let ledger = ShareLedger::load_or_empty(&store.shares_file()).expect("shares");
    let (_, share) = ledger.iter().next().expect("share entry");
    assert_eq!(share.activations, 1, "幂等与拒绝均不重复计数");
}

#[tokio::test]
async fn serve_core_rebuilds_settled_index_from_ledger_json() {
    let (_t, store) = store("serve-rebuild");
    let cfg = cfg_for(&store);
    let keypair = seed_identity(&store, &cfg);
    let receipt = crate::llm_share::tests::common::signed_receipt(&keypair, "r-restart-1");
    p2p_cli::llm_share::ledger::record(&store.data_dir(), &receipt).expect("record");
    let core = ServeCore::new(serve_core_config(
        &store,
        keypair.peer_id().to_string(),
        keypair,
        Arc::new(gate(&store)),
    ));
    let settled = core.settled.lock().await;
    assert!(
        settled.contains_key("r-restart-1"),
        "重启重建 settled 索引（§16.6 #4 防双记账）"
    );
}

#[test]
fn serve_model_provider_unique_mapping_conflict_is_last_error() {
    let provider = |id: &str, models: &[&str]| p2p_cli::llm_share::provider::ProviderView {
        id: id.to_owned(),
        name: id.to_owned(),
        base_url: "https://upstream.invalid/v1".to_owned(),
        protocol: p2p_cli::llm_share::provider::Protocol::OpenAI,
        models: models.iter().map(|m| m.to_string()).collect(),
        created_at: 0,
        api_key_masked: "****".to_owned(),
    };
    let ok = map_models_to_providers(&["gpt-4o".to_owned()], &[provider("a", &["gpt-4o"])])
        .expect("唯一匹配");
    assert_eq!(ok.len(), 1);
    let err = map_models_to_providers(
        &["gpt-4o".to_owned()],
        &[provider("a", &["gpt-4o"]), provider("b", &["gpt-4o"])],
    )
    .unwrap_err();
    assert!(err.contains("唯一映射冲突"), "{err}");
    let err =
        map_models_to_providers(&["gpt-4o".to_owned()], &[provider("b", &["other"])]).unwrap_err();
    assert!(err.contains("无 provider 匹配"), "{err}");
}
#[tokio::test]
async fn serve_proxy_full_path_gates_admits_and_persists_receipt() {
    let (_t, store) = store("serve-proxy");
    let cfg = cfg_for(&store);
    let keypair = seed_identity(&store, &cfg);
    let borrower_dir = store.root().join("borrower-node");
    let borrower = load_or_generate_seed(&borrower_dir.join("key.seed")).expect("borrower seed");
    let g = Arc::new(gate(&store));
    g.allow(
        &borrower.peer_id().to_string(),
        &["gpt-4o".to_owned()],
        None,
        None,
        None,
        "t",
    )
    .expect("allow borrower");
    let request = serde_json::json!({
        "req_id": "r-full-1", "model": "gpt-4o", "max_tokens": 64,
        "messages": [{ "role": "user", "content": "ping" }], "stream": true
    });
    let request_bytes = serde_json::to_vec(&request).expect("json");
    let core = Arc::new(ServeCore::new(serve_core_config(
        &store,
        keypair.peer_id().to_string(),
        keypair.clone(),
        g.clone(),
    )));
    let handler = ProxyGateHandler::new(core);
    let (mut client, server) = tokio::io::duplex(64 * 1024);
    write_chunked(&mut client, &request_bytes)
        .await
        .expect("write request");
    let peer = borrower.peer_id();
    let task = tokio::spawn(async move {
        handler
            .handle_inbound(peer, Box::new(server))
            .await
            .expect("serve");
    });
    let terminal = read_until_terminal(&mut client).await;
    task.await.expect("task");
    assert_eq!(
        terminal["t"], "done",
        "外层闸放行后经内层记账完成: {terminal}"
    );
    let file = p2p_cli::llm_share::ledger::path(&store.data_dir());
    let persisted = p2p_cli::llm_share::ledger::load_or_empty(&file).expect("ledger");
    assert_eq!(persisted.receipts.len(), 1, "收据幂等落盘（§16.6 #4）");
    assert_eq!(persisted.receipts[0].req_id, "r-full-1");
    // 重启模拟：索引重建后同 req_id 重放被 DuplicateReqId 拒（不二次计费）
    let core2 = Arc::new(ServeCore::new(serve_core_config(
        &store,
        keypair.peer_id().to_string(),
        keypair,
        g,
    )));
    let handler2 = ProxyGateHandler::new(core2);
    let (mut client2, server2) = tokio::io::duplex(64 * 1024);
    write_chunked(&mut client2, &request_bytes)
        .await
        .expect("write request");
    let peer2 = borrower.peer_id();
    let task2 = tokio::spawn(async move {
        handler2
            .handle_inbound(peer2, Box::new(server2))
            .await
            .expect("serve replay");
    });
    let terminal2 = read_until_terminal(&mut client2).await;
    task2.await.expect("task2");
    assert_eq!(terminal2["t"], "error", "重放被拒: {terminal2}");
    assert_eq!(terminal2["code"], "duplicate_req_id");
}

async fn read_until_terminal(
    client: &mut (impl tokio::io::AsyncRead + Unpin + Send),
) -> serde_json::Value {
    loop {
        let raw = read_chunked(client).await.expect("frame");
        let value: serde_json::Value = serde_json::from_slice(&raw).expect("json frame");
        if value
            .get("t")
            .and_then(serde_json::Value::as_str)
            .map(|t| t == "done" || t == "error")
            .unwrap_or(false)
        {
            return value;
        }
    }
}
