//! T20 双节点 E2E 夹具（idle-token-sharing-plan §10）：真 facade Node 互联（TCP loopback），
//! 进程内剧本式 mock 上游，全程不出网。服务端对端身份经 ConnectionGate 捕获
//! （底座冻结接缝：ProtocolHandler::handle 只给流，repair-helper 同源裁决），
//! 客户端以 Node 版 StreamFactory 供 ProxyClient 拨号。断言留在 llm_share_wave.rs。

use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use llm_share_proxy::upstream::{SseByteStream, Upstream, UpstreamCall, UpstreamFailure};
use llm_share_proxy::{
    LenderProxy, ModelRoute, ProxyClient, ProxyConfig, ProxyRequest, PROTOCOL_ID,
};
use p2p::{gate_fn, BoxedStream, ConnectionGate, Node, PeerId, ProtocolHandler, ProtocolId};
use p2p_identity::Keypair;
use p2p_protocol::StreamFactory;

/// 单步等待上限：本地 loopback 全链毫秒级，15s 为宽松护栏。
pub const STEP: Duration = Duration::from_secs(15);

/// mock 上游剧本：逐 call 从后往前弹出（多脚本时按逆序传入）。
pub enum Script {
    /// 依次吐出的 SSE 字节块（末块带 usage 则按实收结算）。
    Canned(Vec<Vec<u8>>),
    /// 吐完前缀后流中断且无 usage：断流估算计费路径（A6）。
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

#[async_trait]
impl Upstream for MockUpstream {
    async fn chat(&self, _call: UpstreamCall) -> Result<SseByteStream, UpstreamFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let next = self.script.lock().expect("script lock").pop();
        match next {
            Some(Script::Canned(chunks)) => {
                Ok(futures::stream::iter(chunks.into_iter().map(Ok::<_, UpstreamFailure>)).boxed())
            }
            Some(Script::BrokenAfter(chunks)) => Ok(futures::stream::iter(
                chunks.into_iter().map(Ok::<_, UpstreamFailure>),
            )
            .chain(futures::stream::once(async {
                Err(UpstreamFailure::Broken("mock cut".into()))
            }))
            .boxed()),
            None => Ok(futures::stream::pending().boxed()),
        }
    }
}

/// 入站对端观测：连接门禁在连接建立期捕获对端（最近入站 peer，单桥场景）。
#[derive(Clone, Default)]
pub struct InboundPeers {
    inner: Arc<Mutex<Option<PeerId>>>,
}

impl InboundPeers {
    fn record(&self, peer: PeerId) -> bool {
        match self.inner.lock() {
            Ok(mut slot) => {
                *slot = Some(peer);
                true
            }
            Err(_) => {
                eprintln!("llm_share_e2e: inbound peers lock poisoned; connection denied");
                false
            }
        }
    }

    fn last(&self) -> Option<PeerId> {
        self.inner.lock().ok().and_then(|slot| *slot)
    }

    /// 放行一切连接并在门禁层记录其对端。
    fn gate(&self) -> Arc<dyn ConnectionGate> {
        let this = self.clone();
        Arc::new(gate_fn(move |peer| this.record(*peer)))
    }
}

/// /llm-share/proxy/1 服务端接线：swarm 分发（协议 ID 已消费）后携认证借方喂 serve。
struct ServeHandler {
    proxy: Arc<LenderProxy>,
    peers: InboundPeers,
    protocol: ProtocolId,
}

#[async_trait]
impl ProtocolHandler for ServeHandler {
    fn protocol(&self) -> ProtocolId {
        self.protocol.clone()
    }

    async fn handle(&self, stream: BoxedStream) -> io::Result<()> {
        let borrower = self.peers.last().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotConnected,
                "no inbound peer observed by gate",
            )
        })?;
        self.proxy.serve(stream, borrower).await
    }
}

/// 借方侧拨号工厂：facade Node::new_stream 按 wire-protocol 纪律已写本层协议 ID 首帧，
/// ProxyClient 再写 proxy 层协议 ID 帧；服务端 dispatch_inbound 与 read_request_frame
/// 裸流分支各消费一帧，两层握手恰好对齐。
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

/// 出借方装配：白名单只挂借方；base_url 指向 .invalid 域（mock 在进程内，绝不出网）。
fn proxy_config(
    lender_id: String,
    borrower: &PeerId,
    upstream: Arc<dyn Upstream>,
    net_limit: u64,
) -> ProxyConfig {
    let models = [(
        "gpt-4o".to_string(),
        ModelRoute {
            base_url: "https://upstream.invalid/v1".into(),
            api_key: "sk-test".into(),
            upstream,
        },
    )]
    .into_iter()
    .collect();
    ProxyConfig {
        lender_id,
        period: "2026-09".into(),
        net_limit,
        max_concurrent: 4,
        allowlist: [borrower.to_string()].into_iter().collect(),
        models,
    }
}

/// 双节点夹具：A=出借方（gate 捕获入站身份 + serve 接线），B=借方（拨号工厂）。
/// 身份经种子文件预派生：先读 B 种子拿到 PeerId 供 allowlist 用，再建两端节点。
/// Drop 即关停并清临时目录（造数不落盘过夜）。
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
async fn link(node: &Node, target_peer: PeerId, target: &Node) {
    for addr in target
        .listen_addrs()
        .into_iter()
        .filter(|a| a.contains("/t"))
    {
        node.add_peer_address(target_peer, &addr).expect("addr");
    }
    node.connect(target_peer).await.expect("connect");
}

pub async fn rig(tag: &str, mock: Arc<MockUpstream>, net_limit: u64) -> Rig {
    let _ = p2p_log::init(Default::default());
    let root = std::env::temp_dir().join(format!("llm-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let a_dir = root.join("a");
    let b_dir = root.join("b");
    std::fs::create_dir_all(&a_dir).expect("a dir");
    let keypair = p2p_identity::load_or_generate_seed(&a_dir.join("key.seed")).expect("a seed");
    let b_keypair = {
        std::fs::create_dir_all(&b_dir).expect("b dir");
        p2p_identity::load_or_generate_seed(&b_dir.join("key.seed")).expect("b seed")
    };
    let a = Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(a_dir)
            .build()
            .await
            .expect("node A"),
    );
    let a_peer = a.local_peer_id();
    let b_peer = b_keypair.peer_id();
    let peers = InboundPeers::default();
    a.set_gate(peers.gate());
    let proxy = Arc::new(LenderProxy::new(
        proxy_config(a_peer.to_string(), &b_peer, mock.clone(), net_limit),
        keypair.clone(),
    ));
    a.handle_protocol(Arc::new(ServeHandler {
        proxy: proxy.clone(),
        peers,
        protocol: ProtocolId::new(PROTOCOL_ID).expect("protocol id"),
    }));
    let b = Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(b_dir)
            .build()
            .await
            .expect("node B"),
    );
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

/// 第三节点（A5 非白名单对端）：真实入站连接携带其自身身份。
pub async fn extra_node(tag: &str, name: &str, target: &Rig) -> Arc<Node> {
    let dir = std::env::temp_dir().join(format!("llm-e2e-{tag}-{name}-{}", std::process::id()));
    let node = Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(dir)
            .build()
            .await
            .expect("node"),
    );
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
