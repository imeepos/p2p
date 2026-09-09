//! serve 装配点闸 1 判定源测试（authz-role-design §8/§9，A3 S1）：ServeCore
//! 装配 LenderProxy 注入 with_gate1_authz 后——有绑定（ally）放行全链、
//! 无绑定拒（wire 码同旧路径，message 归因 authz）、绑定表读失败拒（红线 2）、
//! 绑定变更即时生效（每次判定重读，不重装配）。

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use futures::stream::StreamExt;
use llm_share_proxy::server::ModelRoute;
use llm_share_proxy::upstream::{SseByteStream, Upstream, UpstreamCall, UpstreamFailure};
use p2p::ProtocolHandler;
use p2p_authz::{Authz, SystemClock};
use p2p_identity::{load_or_generate_seed, Keypair};
use p2p_protocol::{read_chunked, write_chunked};

use super::common::{cfg_for, seed_identity, store};
use crate::llm_share::serve::gate::AllowlistGate;
use crate::llm_share::serve::proxy::{ProxyGateHandler, ServeCore};
use crate::llm_share::LlmShareStore;

fn gate(store: &LlmShareStore) -> AllowlistGate {
    AllowlistGate::load(&store.data_dir()).expect("gate")
}

/// 外层动态闸放行借方（S1 只切闸 1 判定源，外层 allow/deny 命令面不动）。
fn allow_outer(g: &AllowlistGate, borrower: &str) {
    g.allow(
        borrower,
        &["gpt-4o".to_owned()],
        None,
        None,
        None,
        "s1-test",
    )
    .expect("outer allow");
}

fn bind_ally(store: &LlmShareStore, borrower: &str) {
    Authz::new(Path::new(store.data_dir().as_str()), SystemClock)
        .bind(borrower, "ally", None, "s1-test")
        .expect("bind ally");
}

struct CountingUpstream(AtomicUsize);

type Sse = SseByteStream;

#[async_trait::async_trait]
impl Upstream for CountingUpstream {
    async fn chat(&self, _call: UpstreamCall) -> Result<Sse, UpstreamFailure> {
        self.0.fetch_add(1, Ordering::SeqCst);
        let sse = "data: {}\n\n".as_bytes().to_vec();
        let usage = "data: {\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":5}}\n\n"
            .as_bytes()
            .to_vec();
        Ok(futures::stream::iter(vec![Ok(sse), Ok(usage)]).boxed())
    }
}

fn mock_routes(upstream: Arc<CountingUpstream>) -> HashMap<String, ModelRoute> {
    let mut routes = HashMap::new();
    routes.insert(
        "gpt-4o".to_owned(),
        ModelRoute {
            base_url: "https://upstream.invalid/v1".to_owned(),
            api_key: "sk-test".to_owned(),
            upstream,
        },
    );
    routes
}

fn serve_core_config(
    store: &LlmShareStore,
    lender_id: String,
    keypair: Keypair,
    gate: Arc<AllowlistGate>,
    upstream: Arc<CountingUpstream>,
) -> crate::llm_share::serve::proxy::ServeCoreConfig {
    crate::llm_share::serve::proxy::ServeCoreConfig {
        data_dir: store.data_dir().to_owned(),
        lender_id,
        period: "2026-09-30".to_owned(),
        net_limit: 100_000,
        max_concurrent: 2,
        keypair,
        routes: mock_routes(upstream),
        gate,
    }
}

async fn run_request(
    handler: Arc<ProxyGateHandler>,
    borrower: &Keypair,
    req_id: &str,
) -> serde_json::Value {
    let request = serde_json::json!({
        "req_id": req_id, "model": "gpt-4o", "max_tokens": 64,
        "messages": [{ "role": "user", "content": "ping" }], "stream": true
    });
    let request_bytes = serde_json::to_vec(&request).expect("json");
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
    terminal
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

#[tokio::test]
async fn gate1_bound_ally_passes_full_path_and_persists_receipt() {
    let (_t, store) = store("authz-allow");
    let cfg = cfg_for(&store);
    let keypair = seed_identity(&store, &cfg);
    let borrower_dir = store.root().join("borrower-allow");
    let borrower = load_or_generate_seed(&borrower_dir.join("key.seed")).expect("borrower seed");
    allow_outer(&gate(&store), &borrower.peer_id().to_string());
    bind_ally(&store, &borrower.peer_id().to_string());
    let upstream = Arc::new(CountingUpstream(AtomicUsize::new(0)));
    let core = Arc::new(ServeCore::new(serve_core_config(
        &store,
        keypair.peer_id().to_string(),
        keypair.clone(),
        Arc::new(gate(&store)),
        upstream.clone(),
    )));
    let handler = Arc::new(ProxyGateHandler::new(core));
    let terminal = run_request(handler.clone(), &borrower, "r-authz-allow-1").await;
    assert_eq!(terminal["t"], "done", "绑定 ally 须放行全链: {terminal}");
    assert_eq!(upstream.0.load(Ordering::SeqCst), 1, "上游恰好一次调用");
    let file = p2p_cli::llm_share::ledger::path(&store.data_dir());
    let persisted = p2p_cli::llm_share::ledger::load_or_empty(&file).expect("ledger");
    assert_eq!(persisted.receipts.len(), 1, "放行结算收据落盘");
    assert_eq!(persisted.receipts[0].req_id, "r-authz-allow-1");
}

#[tokio::test]
async fn gate1_unbound_peer_denied_by_authz_with_legacy_wire_code() {
    let (_t, store) = store("authz-unbound");
    let cfg = cfg_for(&store);
    let keypair = seed_identity(&store, &cfg);
    let borrower_dir = store.root().join("borrower-unbound");
    let borrower = load_or_generate_seed(&borrower_dir.join("key.seed")).expect("borrower seed");
    allow_outer(&gate(&store), &borrower.peer_id().to_string());
    let upstream = Arc::new(CountingUpstream(AtomicUsize::new(0)));
    let core = Arc::new(ServeCore::new(serve_core_config(
        &store,
        keypair.peer_id().to_string(),
        keypair.clone(),
        Arc::new(gate(&store)),
        upstream.clone(),
    )));
    let handler = Arc::new(ProxyGateHandler::new(core));
    let terminal = run_request(handler.clone(), &borrower, "r-authz-deny-1").await;
    assert_eq!(terminal["t"], "error", "无绑定必须拒绝: {terminal}");
    assert_eq!(terminal["code"], "not_allowlisted", "wire 码同旧路径");
    let message = terminal["message"].as_str().expect("message");
    assert!(
        message.contains("denied by authz"),
        "判定来源可归因: {message}"
    );
    assert!(
        message.contains("NotBound"),
        "reason 细化在 message: {message}"
    );
    assert_eq!(upstream.0.load(Ordering::SeqCst), 0, "上游零调用");
    let file = p2p_cli::llm_share::ledger::path(&store.data_dir());
    let persisted = p2p_cli::llm_share::ledger::load_or_empty(&file).expect("ledger");
    assert!(persisted.receipts.is_empty(), "拒绝路径零流水");
}

#[tokio::test]
async fn gate1_store_read_failure_denies_with_store_error() {
    let (_t, store) = store("authz-corrupt");
    let cfg = cfg_for(&store);
    let keypair = seed_identity(&store, &cfg);
    let borrower_dir = store.root().join("borrower-corrupt");
    let borrower = load_or_generate_seed(&borrower_dir.join("key.seed")).expect("borrower seed");
    allow_outer(&gate(&store), &borrower.peer_id().to_string());
    bind_ally(&store, &borrower.peer_id().to_string());
    std::fs::write(
        store.root().join("authz").join("bindings.json"),
        b"{not json",
    )
    .expect("corrupt store");
    let core = Arc::new(ServeCore::new(serve_core_config(
        &store,
        keypair.peer_id().to_string(),
        keypair.clone(),
        Arc::new(gate(&store)),
        Arc::new(CountingUpstream(AtomicUsize::new(0))),
    )));
    let handler = Arc::new(ProxyGateHandler::new(core));
    let terminal = run_request(handler.clone(), &borrower, "r-authz-corrupt-1").await;
    assert_eq!(
        terminal["t"], "error",
        "绑定表读失败必须拒绝（红线 2）: {terminal}"
    );
    let message = terminal["message"].as_str().expect("message");
    assert!(
        message.contains("store-error:"),
        "读失败明细入 message: {message}"
    );
}

#[tokio::test]
async fn gate1_binding_change_takes_effect_without_reassembly() {
    let (_t, store) = store("authz-rebind");
    let cfg = cfg_for(&store);
    let keypair = seed_identity(&store, &cfg);
    let borrower_dir = store.root().join("borrower-rebind");
    let borrower = load_or_generate_seed(&borrower_dir.join("key.seed")).expect("borrower seed");
    allow_outer(&gate(&store), &borrower.peer_id().to_string());
    let core = Arc::new(ServeCore::new(serve_core_config(
        &store,
        keypair.peer_id().to_string(),
        keypair.clone(),
        Arc::new(gate(&store)),
        Arc::new(CountingUpstream(AtomicUsize::new(0))),
    )));
    let handler = Arc::new(ProxyGateHandler::new(core));
    let denied = run_request(handler.clone(), &borrower, "r-authz-rebind-1").await;
    assert_eq!(denied["t"], "error", "装配后未绑先拒: {denied}");
    bind_ally(&store, &borrower.peer_id().to_string());
    let allowed = run_request(handler.clone(), &borrower, "r-authz-rebind-2").await;
    assert_eq!(
        allowed["t"], "done",
        "同核心不重装配，补绑定后下一请求即放行（每次判定重读）: {allowed}"
    );
}
