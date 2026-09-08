//! T20 双节点 E2E 夹具（idle-token-sharing-plan §10）：真 facade Node TCP 互联 + 进程内
//! mock 上游，不出网。出借方 ServeHandler 直采 swarm 分发的认证 PeerId
//!（handle_inbound，底座冻结接缝），借方以 Node 版 StreamFactory 拨号。
//! 断言留在 llm_share_wave.rs / llm_share_link_wave.rs。
#![allow(dead_code)] // 共享夹具：各测试二进制按需取用，未取用项非缺陷

use std::collections::{BTreeMap, HashMap};
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use llm_share_offer::{Offer, RateLimit};
use llm_share_proxy::upstream::{SseByteStream, Upstream, UpstreamCall, UpstreamFailure};
use llm_share_proxy::{
    LenderProxy, ModelRoute, ProxyClient, ProxyConfig, ProxyRequest, PROTOCOL_ID,
};
use p2p::{BoxedStream, Node, PeerId, ProtocolHandler, ProtocolId};
use p2p_identity::Keypair;
use p2p_protocol::StreamFactory;

/// 单步等待上限：本地 loopback 全链毫秒级，15s 为宽松护栏。
pub const STEP: Duration = Duration::from_secs(15);

/// mock 上游剧本：逐 call 从后往前弹出；BrokenAfter 吐完前缀即断流无 usage（A6）。
pub enum Script {
    Canned(Vec<Vec<u8>>),
    BrokenAfter(Vec<Vec<u8>>),
}

/// 进程内 mock 上游：记录调用次数（拒绝路径零调用断言的依据），禁外网。
pub struct MockUpstream {
    calls: AtomicUsize,
    script: Mutex<Vec<Script>>,
}

impl MockUpstream {
    pub fn new(script: Vec<Script>) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            script: Mutex::new(script),
        })
    }

    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

/// 把剧本字节块装箱为 SSE 流。
fn sse_stream(chunks: Vec<Vec<u8>>) -> SseByteStream {
    futures::stream::iter(chunks.into_iter().map(Ok::<_, UpstreamFailure>)).boxed()
}

#[async_trait]
impl Upstream for MockUpstream {
    async fn chat(&self, _call: UpstreamCall) -> Result<SseByteStream, UpstreamFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let next = self.script.lock().expect("script lock").pop();
        match next {
            Some(Script::Canned(chunks)) => Ok(sse_stream(chunks)),
            Some(Script::BrokenAfter(chunks)) => Ok(sse_stream(chunks)
                .chain(futures::stream::once(async {
                    Err(UpstreamFailure::Broken("mock cut".into()))
                }))
                .boxed()),
            None => Ok(futures::stream::pending().boxed()),
        }
    }
}

/// /llm-share/proxy/1 服务端接线：handle_inbound 直采握手认证 PeerId，
/// 禁信帧内自报身份；并发入站按流归属，无跨流串线。
struct ServeHandler {
    proxy: Arc<LenderProxy>,
    protocol: ProtocolId,
}

#[async_trait]
impl ProtocolHandler for ServeHandler {
    fn protocol(&self) -> ProtocolId {
        self.protocol.clone()
    }

    async fn handle_inbound(&self, peer: PeerId, stream: BoxedStream) -> io::Result<()> {
        self.proxy.serve(stream, peer).await
    }

    /// 裸流无认证身份上下文：代理记账必须有真实借方，fail-closed 不猜身份。
    async fn handle(&self, _stream: BoxedStream) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "bare stream has no authenticated borrower",
        ))
    }
}

/// 借方侧拨号工厂：facade 与 proxy 层协议 ID 帧各写各消费（read_request_frame
/// 裸流分支承接两层握手，见 wire.rs 接线兼容注释）。
pub struct NodeFactory {
    node: Arc<Node>,
}

#[async_trait]
impl StreamFactory for NodeFactory {
    async fn open_stream(&self, peer: &PeerId, protocol: &ProtocolId) -> io::Result<BoxedStream> {
        self.node
            .new_stream(*peer, protocol.clone())
            .await
            .map_err(|e| io::Error::other(e.to_string()))
    }
}

/// 借方视角客户端：工厂绑定给定节点。
pub fn client_for(node: Arc<Node>) -> ProxyClient<NodeFactory> {
    ProxyClient::new(NodeFactory { node })
}

/// 起一个 mDNS 关闭的测试节点（身份取自 dir 内种子）。
pub async fn spawn_node(dir: PathBuf) -> Arc<Node> {
    let builder = Node::builder().mdns(false).data_dir(dir);
    Arc::new(builder.build().await.expect("node"))
}

/// 单模型上游路由：base_url 指向 .invalid 域（mock 在进程内，绝不出网）。
pub const UPSTREAM_URL: &str = "https://upstream.invalid/v1";

pub fn route(upstream: Arc<dyn Upstream>) -> ModelRoute {
    ModelRoute {
        base_url: UPSTREAM_URL.into(),
        api_key: "sk-test".into(),
        upstream,
    }
}

/// 单模型 offer 样例（A5 过期断言等共用）：TTL 1s，周期末 2026-09-30。
pub fn sample_offer(peer: &PeerId) -> Offer {
    Offer {
        peer: peer.to_string(),
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
    }
}

/// 出借方装配：白名单 = 主借方 + 测试自备对端。
fn proxy_config(
    lender_id: String,
    allowlist: &[String],
    models: HashMap<String, ModelRoute>,
    net_limit: u64,
) -> ProxyConfig {
    ProxyConfig {
        lender_id,
        period: "2026-09".into(),
        net_limit,
        max_concurrent: 4,
        allowlist: allowlist.iter().cloned().collect(),
        models,
    }
}

/// 双节点夹具：A=出借方（handle_inbound 认证身份 + serve 接线），B=借方；身份经
/// 种子文件预派生（先读 B 种子拿 PeerId 供 allowlist）。Drop 即关停并清临时目录。
pub struct Rig {
    pub a: Arc<Node>,
    pub keypair: Keypair,
    pub proxy: Arc<LenderProxy>,
    pub client: ProxyClient<NodeFactory>,
    pub b: Arc<Node>,
    pub a_peer: PeerId,
    pub b_peer: PeerId,
    root: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.a.shutdown();
        self.b.shutdown();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// 登记对端 TCP 地址并建连（真链路互联；地址簿 + connect 同 chat_e2e 先例）。
pub async fn link(node: &Node, target_peer: PeerId, target: &Node) {
    for addr in target
        .listen_addrs()
        .into_iter()
        .filter(|a| a.contains("/t"))
    {
        node.add_peer_address(target_peer, &addr).expect("addr");
    }
    node.connect(target_peer).await.expect("connect");
}

/// 通用装配：自定义模型路由 + 额外白名单（B 波双协议/多借方复用）。
pub async fn rig_custom(
    tag: &str,
    models: HashMap<String, ModelRoute>,
    extra_allow: Vec<String>,
    net_limit: u64,
) -> Rig {
    let _ = p2p_log::init(Default::default());
    let root = std::env::temp_dir().join(format!("llm-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let a_dir = root.join("a");
    let b_dir = root.join("b");
    std::fs::create_dir_all(&a_dir).expect("a dir");
    let keypair = p2p_identity::load_or_generate_seed(&a_dir.join("key.seed")).expect("a seed");
    std::fs::create_dir_all(&b_dir).expect("b dir");
    let b_keypair = p2p_identity::load_or_generate_seed(&b_dir.join("key.seed")).expect("b seed");
    let a = spawn_node(a_dir).await;
    let a_peer = a.local_peer_id();
    let b_peer = b_keypair.peer_id();
    let mut allow = vec![b_peer.to_string()];
    allow.extend(extra_allow);
    let proxy = Arc::new(LenderProxy::new(
        proxy_config(a_peer.to_string(), &allow, models, net_limit),
        keypair.clone(),
    ));
    a.handle_protocol(Arc::new(ServeHandler {
        proxy: proxy.clone(),
        protocol: ProtocolId::new(PROTOCOL_ID).expect("protocol id"),
    }));
    let b = spawn_node(b_dir).await;
    assert_eq!(b.local_peer_id(), b_peer, "B 身份须与预派生种子一致");
    link(&b, a_peer, &a).await;
    Rig {
        a,
        keypair,
        proxy,
        client: client_for(b.clone()),
        b,
        a_peer,
        b_peer,
        root,
    }
}

/// A 波单模型装配：mock 上游单路由，白名单只有借方。
pub async fn rig(tag: &str, mock: Arc<MockUpstream>, net_limit: u64) -> Rig {
    let models = HashMap::from([("gpt-4o".to_string(), route(mock))]);
    rig_custom(tag, models, Vec::new(), net_limit).await
}

/// 第三节点（A5 非白名单对端）：真实入站连接携带其自身身份。
pub async fn extra_node(tag: &str, name: &str, target: &Rig) -> Arc<Node> {
    let dir = std::env::temp_dir().join(format!("llm-e2e-{tag}-{name}-{}", std::process::id()));
    let node = spawn_node(dir).await;
    link(&node, target.a_peer, &target.a).await;
    node
}

/// B 拨 A 发起一次代理调用；收据验签失败在 client 侧显式报错不吞。
pub async fn call(rig: &Rig, req: &ProxyRequest) -> Vec<llm_share_proxy::ProxyEvent> {
    rig.client
        .call(rig.a_peer, req, rig.keypair.public(), STEP)
        .await
        .expect("proxy call")
}

/// OpenAI chat completions 请求体（含代理闸门必需字段 req_id/model/max_tokens）。
pub fn proxy_request(req_id: &str, model: &str, max_tokens: u64) -> ProxyRequest {
    let body = serde_json::json!({
        "req_id": req_id, "model": model, "max_tokens": max_tokens,
        "messages": [{ "role": "user", "content": "ping" }], "stream": true
    });
    ProxyRequest::parse(&serde_json::to_vec(&body).expect("json")).expect("valid request")
}

/// 单个 SSE data 事件字节。
pub fn sse_data(payload: &str) -> Vec<u8> {
    format!("data: {payload}\n\n").into_bytes()
}

/// 上游账单 chunk：OpenAI 流末 usage 事件。
pub fn usage_chunk(prompt: u64, completion: u64) -> Vec<u8> {
    sse_data(&format!(
        "{{\"usage\":{{\"prompt_tokens\":{prompt},\"completion_tokens\":{completion}}}}}"
    ))
}
