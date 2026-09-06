//! borrow 进程内夹具（F11/PR6 三场景）：真 facade Node TCP 互联 + 进程内 mock
//! 上游（不出网），接线与 p2p-itest llm_share_common 同源；出借方 allowlist 可控。

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use futures::StreamExt;
use llm_share_offer::{Offer, RateLimit, SignedOffer, PROTOCOL_ID as OFFER_PROTOCOL};
use llm_share_proxy::upstream::{SseByteStream, Upstream, UpstreamCall, UpstreamFailure};
use llm_share_proxy::{LenderProxy, ModelRoute, ProxyConfig, PROTOCOL_ID};
use p2p::{gate_fn, BoxedStream, ConnectionGate, Node, PeerId, ProtocolHandler, ProtocolId};
use p2p_identity::{load_or_generate_seed, Keypair};
use p2p_protocol::{read_chunked, write_chunked};

use p2p_cli::llm_share::borrow::BorrowParams;

pub const MODEL: &str = "gpt-4o";
pub const PERIOD: &str = "2026-09";

/// mock 上游剧本：逐 call 弹出；BrokenAfter 吐完前缀即断流无 usage（A6 入口）。
pub enum Script {
    Canned(Vec<Vec<u8>>),
    BrokenAfter(Vec<Vec<u8>>),
}

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

fn sse_stream(chunks: Vec<Vec<u8>>) -> SseByteStream {
    futures::stream::iter(chunks.into_iter().map(Ok::<_, UpstreamFailure>)).boxed()
}

#[async_trait]
impl Upstream for MockUpstream {
    async fn chat(&self, _call: UpstreamCall) -> Result<SseByteStream, UpstreamFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.script.lock().expect("script lock").pop() {
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

/// 入站对端观测：serve 侧认证借方身份取自安全层（经门禁捕获，单桥场景）。
#[derive(Clone, Default)]
pub struct InboundPeers {
    inner: Arc<Mutex<Option<PeerId>>>,
}

impl InboundPeers {
    pub fn gate(&self) -> Arc<dyn ConnectionGate> {
        let this = self.clone();
        Arc::new(gate_fn(move |peer| {
            if let Ok(mut slot) = this.inner.lock() {
                *slot = Some(*peer);
            }
            true
        }))
    }

    fn last(&self) -> Option<PeerId> {
        self.inner.lock().ok().and_then(|slot| *slot)
    }
}

/// /llm-share/offer/1 应答：协议 ID 首帧跳过后回 SignedOffer 信封 JSON。
struct OfferHandler {
    signed: Arc<SignedOffer>,
}

#[async_trait]
impl ProtocolHandler for OfferHandler {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(OFFER_PROTOCOL).expect("offer protocol id")
    }

    async fn handle(&self, mut stream: BoxedStream) -> io::Result<()> {
        let first = read_chunked(&mut stream).await?;
        if first.first() == Some(&b'/') {
            read_chunked(&mut stream).await?;
        }
        let payload = serde_json::to_vec(self.signed.as_ref())?;
        write_chunked(&mut stream, &payload).await
    }
}

/// /llm-share/proxy/1 服务端接线：喂已认证借方身份给 LenderProxy::serve。
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

/// 双节点夹具：lender 节点（offer+proxy 接线、allowlist 可控）+ 借方身份种子目录。
/// Drop 即关停 lender 并清临时目录。
pub struct Rig {
    pub root: PathBuf,
    pub borrower_dir: PathBuf,
    pub ledger_dir: PathBuf,
    pub lender: Arc<Node>,
    pub keypair: Keypair,
    pub borrower_peer: String,
    pub mock: Arc<MockUpstream>,
}

impl Rig {
    pub fn ledger_dir_str(&self) -> String {
        self.ledger_dir.display().to_string()
    }

    pub fn ledger_path(&self) -> PathBuf {
        self.ledger_dir.join("llm-share").join("ledger.json")
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.lender.shutdown();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 出借方装配：allowlist 依 allow_borrower 开关，单模型路由挂 mock 上游。
pub async fn rig(tag: &str, script: Vec<Script>, allow_borrower: bool) -> Rig {
    let root = std::env::temp_dir().join(format!("llm-borrow-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let borrower_dir = root.join("borrower");
    let lender_dir = root.join("lender");
    std::fs::create_dir_all(&borrower_dir).expect("borrower dir");
    std::fs::create_dir_all(&lender_dir).expect("lender dir");
    let borrower_seed =
        load_or_generate_seed(&borrower_dir.join("key.seed")).expect("borrower seed");
    let borrower_peer = borrower_seed.peer_id().to_string();
    let keypair = load_or_generate_seed(&lender_dir.join("key.seed")).expect("lender seed");
    let node = Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(lender_dir.clone())
            .build()
            .await
            .expect("lender node"),
    );
    let peers = InboundPeers::default();
    node.set_gate(peers.gate());
    let mock = MockUpstream::new(script);
    let allowlist: HashSet<String> = if allow_borrower {
        [borrower_peer.clone()].into_iter().collect()
    } else {
        HashSet::new()
    };
    let cfg = ProxyConfig {
        lender_id: node.local_peer_id().to_string(),
        period: PERIOD.into(),
        net_limit: 1_000_000,
        max_concurrent: 4,
        allowlist,
        models: HashMap::from([(
            MODEL.to_owned(),
            ModelRoute {
                base_url: "https://upstream.invalid/v1".into(),
                api_key: "sk-test".into(),
                upstream: mock.clone(),
            },
        )]),
    };
    let proxy = Arc::new(LenderProxy::new(cfg, keypair.clone()));
    node.handle_protocol(Arc::new(OfferHandler {
        signed: Arc::new(signed_offer(&keypair, &node.local_peer_id().to_string())),
    }));
    node.handle_protocol(Arc::new(ServeHandler {
        proxy,
        peers,
        protocol: ProtocolId::new(PROTOCOL_ID).expect("proxy protocol id"),
    }));
    Rig {
        ledger_dir: root.join("ledger"),
        root,
        borrower_dir,
        lender: node,
        keypair,
        borrower_peer,
        mock,
    }
}

/// 以出借方身份签发单模型声明（TTL 1h，单请求上限 128）。
fn signed_offer(keypair: &Keypair, peer: &str) -> SignedOffer {
    let offer = Offer {
        peer: peer.to_owned(),
        models: vec![MODEL.to_owned()],
        spare: [(MODEL.to_owned(), 100_000u64)].into_iter().collect(),
        period_ends: "2026-12-31".into(),
        max_per_req: [(MODEL.to_owned(), 128u64)].into_iter().collect(),
        rate_limit: RateLimit {
            rpm: 60,
            concurrency: 4,
        },
        ttl_secs: 3_600,
        retention: "none".into(),
    };
    SignedOffer::sign(&offer, keypair, now_secs()).expect("sign offer")
}

/// 借方直连参数：--addr 走 lender 监听地址，bootstrap 留空（不走查号）。
pub fn params(rig: &Rig, req_id: &str) -> BorrowParams {
    let addr = rig
        .lender
        .listen_addrs()
        .into_iter()
        .find(|a| a.contains("/t"))
        .expect("lender listen addr");
    BorrowParams {
        lender: rig.lender.local_peer_id().to_string(),
        model: Some(MODEL.to_owned()),
        prompt: Some("ping".into()),
        messages_json: None,
        max_tokens: Some(64),
        addr: Some(addr),
        bootstrap: vec![],
        node_dir: rig.borrower_dir.display().to_string(),
        ledger_dir: rig.ledger_dir_str(),
        timeout_secs: 15,
        discover_secs: 1,
        req_id: Some(req_id.to_owned()),
    }
}

pub fn lender_pubkey(rig: &Rig) -> String {
    bs58::encode(rig.keypair.public()).into_string()
}

/// 正常上游剧本：内容帧 + usage 帧 + [DONE]。
pub fn canned_ok() -> Vec<Vec<u8>> {
    vec![
        sse_data("{\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}"),
        sse_data("{\"usage\":{\"prompt_tokens\":21,\"completion_tokens\":9}}"),
        sse_data("[DONE]"),
    ]
}

/// 断流剧本：只有内容帧，usage 前即断。
pub fn content_only() -> Vec<Vec<u8>> {
    vec![sse_data(
        "{\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}",
    )]
}

fn sse_data(payload: &str) -> Vec<u8> {
    format!("data: {payload}\n\n").into_bytes()
}
