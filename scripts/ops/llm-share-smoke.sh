#!/usr/bin/env bash
# llm-share 真两机联调冒烟（T23）：Mac(本机) ⇄ 102(Linux) 两台自有节点。
#
# 合规注记（idle-token-sharing-plan §8 / R6 边界内）：本脚本仅用于两个自有节点
# 白名单互助的测试验证——朋友间闲量互助、成本共担，barter 记账不可提现、
# 不可转让、无充值/变现/公开经营语义；上游为进程内 mock，绝不出网。
#
# 场景（逐场景输出 PASS/FAIL，末行全绿输出 SMOKE-OK，任一 FAIL 即失败退出）：
#   S1 远端供给   探测 102 rust 工具链（缺失则 rustup 装至 $HOME，禁 sudo）；
#                 git bundle 克隆/增量更新到 102 ~/llm-smoke-work/src 后构建
#                 harness 与 p2pctl；本地同构构建（首次供给慢属预期）。
#   S2 互联       双节点经既有底座 rendezvous/bootstrap 发现并建立连接。
#   S3 出借方=102 102 侧以节点身份 p2pctl offer publish 签名发布声明，
#                 本机借方经 /llm-share/offer/1 取回验签（Ed25519+TTL）并选路。
#   S4 跨机调用   本机借方经 102 代理发起真实流式 chat（上游=102 侧进程内 mock），
#                 双边账本各记一笔，本机收据经产品 CLI（p2pctl llm-share receipt
#                 verify）Ed25519 验签 PASS。
#   S5 负向       非 allowlist peer 的代理请求被结构化拒绝（NotAllowlisted），
#                 上游调用计数增量为零。
#   S6 可观察     坏节点路径显式报错不挂死（S6a）；断流路径显式 STREAM-BROKEN
#                 + estimated 收据不挂死（S6b）；整脚本总超时 1800s 自毁。
#
# 纪律：幂等（连跑两次全绿）；清理（本地 llm-smoke-* 与远端 ~/llm-smoke-work，
# trap 含失败路径全清理；元数据日志可留，prompt 内容禁止落盘——mock 回答与
# 日志仅含模型名/token 数等元数据）；密钥只经 .env/环境变量（本流程用进程内
# mock 上游，无需任何真实 API key）。
#
# 数据面接线说明：/llm-share/proxy 与 /llm-share/offer 的进程级装配（底座
# handler 注册）属产品待接线面，本脚本以独立 harness（脚本生成，路径依赖产品
# crate llm-share-proxy/offer/ledger + p2p facade，不改产品代码）承担两机进程
# 载体；签名/验签/账本/收据/选路全部为产品 crate 真实逻辑。
#
# 用法：bash scripts/ops/llm-share-smoke.sh
#   可覆盖：LLM_SMOKE_REMOTE（默认 imeepos@192.168.0.102）
#           LLM_SMOKE_REMOTE_IP（默认 192.168.0.102）
set -euo pipefail
export RUST_LOG="${RUST_LOG:-warn}"
export PATH="/opt/homebrew/bin:${HOME}/.cargo/bin:${PATH}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REMOTE="${LLM_SMOKE_REMOTE:-imeepos@192.168.0.102}"
REMOTE_IP="${LLM_SMOKE_REMOTE_IP:-192.168.0.102}"
WORK="${TMPDIR:-/tmp}/llm-smoke-run-$$"
HARNESS_DIR="${TMPDIR:-/tmp}/llm-smoke-harness"
REMOTE_WORK="llm-smoke-work"
BOOT_QUIC=35410
LEND_QUIC=35420
OBS_PORT=$((BOOT_QUIC + 2))
MODEL="smoke-model"
PERIOD="$(date -u +%Y-%m)"
PERIOD_ENDS="$(date -u +%Y)-12-31"
PASS_LIST=()
LOCAL_PIDS=()

log() { printf '%s\n' "$*"; }
sc_pass() { PASS_LIST+=("$1"); log "${1} PASS ${2}"; }
dump_diag() {
    log "---- 诊断: 本地 bootstrap.log 尾部"
    tail -12 "${WORK}/boot.log" 2>/dev/null || true
}
sc_fail() { dump_diag; log "${1} FAIL ${2}"; exit 1; }
die() { log "llm-share-smoke: $*"; exit 2; }

# 超时工具：本机用 gtimeout（brew coreutils）；远端 Debian 用同义 timeout。
gt() { gtimeout "$@"; }
remote_sh() { gt 120 ssh -o BatchMode=yes -o ConnectTimeout=10 "${REMOTE}" "$@"; }

cleanup() {
    local rc=$?
    trap - EXIT TERM INT
    if [ -n "${WATCHDOG_PID:-}" ]; then kill "${WATCHDOG_PID}" 2>/dev/null || true; fi
    local pid
    for pid in "${LOCAL_PIDS[@]:-}"; do
        [ -n "${pid}" ] && kill "${pid}" 2>/dev/null || true
    done
    if [ -n "${REMOTE_WORK:-}" ] && [ "${REMOTE_SUPPLIED:-0}" = "1" ]; then
        gt 30 ssh -o BatchMode=yes -o ConnectTimeout=10 "${REMOTE}" \
            'for f in llm-smoke-work/run/*.pid; do [ -e "$f" ] && kill "$(cat "$f")" 2>/dev/null; done; rm -rf llm-smoke-work/run llm-smoke-work/lender llm-smoke-work/logs' \
            >/dev/null 2>&1 || true
    fi
    [ -n "${WORK:-}" ] && rm -rf "${WORK}"
    exit "${rc}"
}
on_term() {
    log "SMOKE-ABORT 收到 TERM（总超时或外部终止）"
    exit 124
}
trap cleanup EXIT
trap on_term TERM INT

# 总超时看门狗：1740s 预警自毁（含清理），低于建议上限 1800s。
( sleep 1740 && kill -TERM "$$" ) >/dev/null 2>&1 &
WATCHDOG_PID=$!

# ---- harness 生成与构建（脚本生成的独立测试夹具，不入库）----

gen_harness() {
    # $1 = harness 目录, $2 = 仓库 crates 目录（路径依赖，不改产品代码）
    dir="${1}"
    crates="${2}"
    mkdir -p "${dir}/src"
    cat > "${dir}/Cargo.toml" <<TOML
[package]
name = "llm-smoke-harness"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
bs58 = "0.5"
futures = "0.3"
async-trait = "0.1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
p2p = { path = "${crates}/p2p" }
p2p-identity = { path = "${crates}/p2p-identity" }
p2p-protocol = { path = "${crates}/p2p-protocol" }
llm-share-offer = { path = "${crates}/llm-share-offer" }
llm-share-ledger = { path = "${crates}/llm-share-ledger" }
llm-share-proxy = { path = "${crates}/llm-share-proxy" }
TOML
    cat > "${dir}/src/main.rs" <<'RUST_EOF'

//! llm-smoke 两机联调夹具（由 llm-share-smoke.sh 生成，不入库）：
//! bootstrap / serve / call / gen-peer 四模式。
//! serve=出借方（offer 信封经 /llm-share/offer/1 应答 + /llm-share/proxy/1 代理 +
//! 进程内 mock 上游，上游调用计数落元数据日志）；call=借方（rendezvous 发现 ->
//! offer 验签选路 -> 代理流式调用 -> 收据验签入账落盘）。

use std::collections::{HashMap, HashSet};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;

use llm_share_ledger::{Ledger, Receipt};
use llm_share_offer::{select_offers, OfferBook, SignedOffer, PROTOCOL_ID as OFFER_PROTOCOL};
use llm_share_proxy::upstream::{SseByteStream, Upstream, UpstreamCall, UpstreamFailure};
use llm_share_proxy::{
    ErrorCode, LenderProxy, ModelRoute, ProxyClient, ProxyConfig, ProxyEvent, ProxyRequest,
};
use p2p::{gate_fn, BoxedStream, ConnectionGate, Node, PeerId, ProtocolHandler, ProtocolId};
use p2p_identity::{load_or_generate_seed, Keypair};
use p2p_protocol::{read_chunked, write_chunked, StreamFactory};

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn die(what: &str) -> ! {
    eprintln!("HARNESS-FAIL: {what}");
    std::process::exit(2);
}

/// base58 PeerId 解析（对齐 facade parse_peer_id 规则：解码后恰 32 字节）。
fn parse_peer(s: &str) -> PeerId {
    let raw = bs58::decode(s).into_vec().unwrap_or_default();
    let arr: [u8; 32] = raw.try_into().unwrap_or([0u8; 32]);
    PeerId::from_bytes(arr)
}

/// --key value 形态参数解析（不引 clap，保持夹具轻量）。
struct Args(HashMap<String, String>);

impl Args {
    fn parse() -> Self {
        let mut map = HashMap::new();
        let argv: Vec<String> = std::env::args().skip(1).collect();
        map.insert("mode".into(), argv.first().cloned().unwrap_or_default());
        let mut i = 1;
        while i < argv.len() {
            let Some(k) = argv[i].strip_prefix("--") else {
                i += 1;
                continue;
            };
            // 无值形态（布尔开关，如 --probe）：值缺省或以下个 --flag 开头即视为开关。
            let takes_value = i + 1 < argv.len() && !argv[i + 1].starts_with("--");
            if takes_value {
                map.insert(k.into(), argv[i + 1].clone());
                i += 2;
            } else {
                map.insert(k.into(), "1".into());
                i += 1;
            }
        }
        Self(map)
    }
    fn get(&self, k: &str) -> &str {
        self.0.get(k).map(String::as_str).unwrap_or("")
    }
    fn num(&self, k: &str) -> u16 {
        self.get(k)
            .parse()
            .unwrap_or_else(|_| die(&format!("--{k} 非法")))
    }
    fn has(&self, k: &str) -> bool {
        self.0.contains_key(k)
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let args = Args::parse();
    match args.get("mode") {
        "gen-peer" => gen_peer(),
        "identity" => identity(&args),
        "bootstrap" => bootstrap(&args).await,
        "serve" => serve(&args).await,
        "call" => call(&args).await,
        "buildtest" => {
            let node = build_node(&args, 0).await;
            println!("BUILDTEST-OK peer={}", node.local_peer_id());
        }
        other => die(&format!("unknown mode: {other}")),
    }
}

fn gen_peer() {
    println!("peerId={}", Keypair::generate().peer_id());
}

/// 确保身份种子存在并输出本机 PeerId（allowlist 预登记依赖：借方身份先于出借方 serve 存在）。
fn identity(args: &Args) {
    let seed = PathBuf::from(args.get("data")).join("key.seed");
    let kp = load_or_generate_seed(&seed).unwrap_or_else(|e| die(&format!("seed: {e}")));
    println!("peerId={}", kp.peer_id());
}

async fn build_node(args: &Args, quic: u16) -> Node {
    let boot = args.get("bootstrap");
    let mut b = Node::builder()
        .mdns(false)
        .data_dir(PathBuf::from(args.get("data")))
        .quic_port(quic);
    if !boot.is_empty() {
        b = b.bootstrap(vec![boot.to_owned()]);
    }
    let obs = args.get("observation");
    if !obs.is_empty() {
        b = b.observation_addrs(vec![obs.to_owned()]);
    }
    b.build().await.unwrap_or_else(|e| {
        die(&format!("node build (data={} quic={quic} boot={boot} obs={}): {e:?}", args.get("data"), args.get("observation")))
    })
}

async fn bootstrap(args: &Args) {
    let quic = args.num("quic");
    let node = Node::builder()
        .mdns(false)
        .data_dir(PathBuf::from(args.get("data")))
        .quic_port(quic)
        .observation_responder(quic.saturating_add(2))
        .build()
        .await
        .unwrap_or_else(|e| die(&format!("bootstrap node build: {e}")));
    println!("peerId={}", node.local_peer_id());
    println!("BOOTSTRAP-READY quic={quic}");
    tokio::signal::ctrl_c().await.ok();
}

/// 入站对端观测：连接门禁捕获最近入站 peer（底座冻结接缝，与产品 itest 同源）。
#[derive(Default, Clone)]
struct InboundPeers {
    inner: Arc<Mutex<Option<PeerId>>>,
}

impl InboundPeers {
    fn gate(&self) -> Arc<dyn ConnectionGate> {
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

/// 进程内 mock 上游：逐调用记元数据日志（序号/模型名，非 prompt 内容），
/// 第 N 次调用按剧本断流（S6b）。
struct MockUpstream {
    calls: AtomicUsize,
    broken_on: Option<usize>,
    log: PathBuf,
}

#[async_trait]
impl Upstream for MockUpstream {
    async fn chat(&self, call: UpstreamCall) -> Result<SseByteStream, UpstreamFailure> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&self.log) {
            let _ = writeln!(f, "{{\"ts\":{},\"call\":{},\"model\":\"{}\"}}", now_secs(), n, call.model);
        }
        let payloads = [
            "{\"choices\":[{\"delta\":{\"content\":\"smoke-ok\"}}]}",
            "{\"usage\":{\"prompt_tokens\":21,\"completion_tokens\":9}}",
            "[DONE]",
        ];
        // 上游流须为 SSE 事件原文（data: 行 + 空行边界），对齐 itest sse_data 口径
        let chunks: Vec<Result<Vec<u8>, UpstreamFailure>> = payloads
            .iter()
            .map(|p| Ok(format!("data: {p}\n\n").into_bytes()))
            .collect();
        if self.broken_on == Some(n) {
            // 断流剧本：只吐内容帧、usage 帧前即断（A6：无 usage 才产生 estimated 收据）
            let cut: Vec<Result<Vec<u8>, UpstreamFailure>> = vec![
                Ok(format!("data: {}\n\n", payloads[0]).into_bytes()),
                Err(UpstreamFailure::Broken("smoke-mock-cut".into())),
            ];
            return Ok(futures::stream::iter(cut).boxed());
        }
        Ok(futures::stream::iter(chunks).boxed())
    }
}

struct OfferHandler {
    signed: Arc<SignedOffer>,
}

#[async_trait]
impl ProtocolHandler for OfferHandler {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(OFFER_PROTOCOL).expect("offer protocol id")
    }
    async fn handle(&self, mut stream: BoxedStream) -> std::io::Result<()> {
        let first = read_chunked(&mut stream).await?;
        if first.first() == Some(&b'/') {
            read_chunked(&mut stream).await?;
        }
        let payload = serde_json::to_vec(self.signed.as_ref())?;
        write_chunked(&mut stream, &payload).await
    }
}

struct ProxyHandler {
    proxy: Arc<LenderProxy>,
    peers: InboundPeers,
    period: String,
}

#[async_trait]
impl ProtocolHandler for ProxyHandler {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(llm_share_proxy::PROTOCOL_ID).expect("proxy protocol id")
    }
    async fn handle(&self, stream: BoxedStream) -> std::io::Result<()> {
        let Some(borrower) = self.peers.last() else {
            return Err(std::io::Error::new(ErrorKind::NotConnected, "no inbound peer observed"));
        };
        let done = self.proxy.serve(stream, borrower).await;
        let ledger = self.proxy.ledger().await;
        let receipts = self.proxy.receipts().await;
        println!(
            "SERVE-LEDGER borrower={} receipts={} net={}",
            borrower,
            receipts.len(),
            ledger.net(&borrower.to_string(), &self.period)
        );
        done
    }
}

async fn serve(args: &Args) {
    let quic = args.num("quic");
    let node = Arc::new(build_node(args, quic).await);
    let seed = PathBuf::from(args.get("data")).join("key.seed");
    let kp = load_or_generate_seed(&seed).unwrap_or_else(|e| die(&format!("seed: {e}")));
    let peer_str = node.local_peer_id().to_string();
    println!("peerId={peer_str}");

    let text = std::fs::read_to_string(args.get("offer-file"))
        .unwrap_or_else(|e| die(&format!("offer file: {e}")));
    let signed: SignedOffer =
        serde_json::from_str(&text).unwrap_or_else(|e| die(&format!("offer json: {e}")));
    if signed.offer.peer != peer_str {
        die("offer.peer 与本机身份不一致（声明须以本机节点身份签发）");
    }
    if signed.verify(now_secs()).is_err() {
        die("offer 验签失败（TTL 已过或签名损坏）");
    }

    let allow: HashSet<String> = args.get("allow").split(',').map(str::to_owned).collect();
    let mock = Arc::new(MockUpstream {
        calls: AtomicUsize::new(0),
        broken_on: args.get("broken-on").parse().ok(),
        log: PathBuf::from(args.get("upstream-log")),
    });
    let cfg = ProxyConfig {
        lender_id: peer_str.clone(),
        period: args.get("period").to_owned(),
        net_limit: args.get("net-limit").parse().unwrap_or(1_000_000),
        max_concurrent: 4,
        allowlist: allow,
        models: HashMap::from([(
            args.get("model").to_owned(),
            ModelRoute {
                base_url: "https://upstream.invalid/v1".into(),
                api_key: "sk-smoke".into(),
                upstream: mock,
            },
        )]),
    };
    let proxy = Arc::new(LenderProxy::new(cfg, kp));
    let peers = InboundPeers::default();
    node.set_gate(peers.gate());
    node.handle_protocol(Arc::new(OfferHandler { signed: Arc::new(signed) }));
    node.handle_protocol(Arc::new(ProxyHandler {
        proxy,
        peers,
        period: args.get("period").to_owned(),
    }));
    println!("SERVE-READY quic={quic}");
    // 自查询保活：维持 rendezvous 链路活跃；注册空白窗口期自观测留痕
    {
        let kn = node.clone();
        let kp_str = peer_str.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                match kn.query_peer(&kp_str).await {
                    Ok(addrs) if !addrs.is_empty() => {}
                    _ => tracing::warn!("self-query empty: rendezvous 注册可能存在空白窗口"),
                }
            }
        });
    }
    tokio::signal::ctrl_c().await.ok();
}

struct NodeFactory {
    node: Arc<Node>,
}

#[async_trait]
impl StreamFactory for NodeFactory {
    async fn open_stream(&self, peer: &PeerId, protocol: &ProtocolId) -> std::io::Result<BoxedStream> {
        self.node
            .new_stream(*peer, protocol.clone())
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))
    }
}

fn fail_exit(code: i32, what: &str) -> ! {
    eprintln!("HARNESS-FAIL: {what}");
    std::process::exit(code);
}

async fn discover(node: &Node, lender: PeerId, wait_secs: u64) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(wait_secs);
    loop {
        match node.query_peer(&lender.to_string()).await {
            Ok(addrs) if !addrs.is_empty() => {
                println!("DISCOVER-OK addr={}", addrs[0]);
                return addrs[0].clone();
            }
            _ => {
                if tokio::time::Instant::now() >= deadline {
                    // 优雅退出：先 shutdown 让对端收到连接关闭，避免 liveness 探测打在死连接上
                    return String::new();
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
}

async fn fetch_offer(node: &Arc<Node>, peer: PeerId) -> SignedOffer {
    let id = ProtocolId::new(OFFER_PROTOCOL).expect("offer protocol id");
    let mut stream = node
        .new_stream(peer, id)
        .await
        .unwrap_or_else(|e| fail_exit(4, &format!("offer stream: {e}")));
    write_chunked(&mut stream, b"get")
        .await
        .unwrap_or_else(|e| fail_exit(4, &format!("offer req: {e}")));
    let first = read_chunked(&mut stream)
        .await
        .unwrap_or_else(|e| fail_exit(4, &format!("offer read: {e}")));
    let payload = if first.first() == Some(&b'/') {
        read_chunked(&mut stream)
            .await
            .unwrap_or_else(|e| fail_exit(4, &format!("offer read: {e}")))
    } else {
        first
    };
    serde_json::from_slice(&payload).unwrap_or_else(|e| fail_exit(4, &format!("offer json: {e}")))
}

async fn call(args: &Args) {
    let node = Arc::new(build_node(args, 0).await);
    let lender = parse_peer(args.get("lender"));
    let wait: u64 = args.get("discover-wait").parse().unwrap_or(10);
    let mut addr = match args.get("lender-addr") {
        "" => discover(node.as_ref(), lender, wait).await,
        direct => direct.to_owned(),
    };
    if addr.is_empty() {
        node.shutdown();
        fail_exit(2, "DISCOVER-FAIL: rendezvous 查号超时（对端未注册或已过期）");
    }
    node.add_peer_address(lender, &addr)
        .unwrap_or_else(|e| fail_exit(3, &format!("bad addr: {e}")));
    node.connect(lender)
        .await
        .unwrap_or_else(|e| fail_exit(3, &format!("CONNECT-FAIL: {e}")));
    println!("CONNECT-OK");

    let signed = fetch_offer(&node, lender).await;
    let now = now_secs();
    let mut book = OfferBook::new();
    if let Err(e) = book.insert(signed.clone(), now) {
        fail_exit(4, &format!("OFFER-VERIFY-FAIL: {e}"));
    }
    let model = args.get("model").to_owned();
    let cand = select_offers(book.live(now), &model, now)
        .into_iter()
        .find(|c| c.peer == signed.offer.peer)
        .unwrap_or_else(|| fail_exit(4, "OFFER-ROUTE-FAIL: 无有效候选（模型无闲量或 TTL 过期）"));
    println!(
        "OFFER-OK pubkey={} ttl_left={} spare={}",
        bs58::encode(signed.pubkey).into_string(),
        signed.expires_at().saturating_sub(now),
        cand.spare
    );
    if args.has("probe") {
        println!("PROBE-OK");
        node.shutdown();
        return;
    }

    let max_tokens: u64 = args.get("max-tokens").parse().unwrap_or(64);
    let body = serde_json::json!({
        "model": model,
        "stream": true,
        "max_tokens": max_tokens,
        "messages": [{"role": "user", "content": "smoke"}]
    });
    let req = ProxyRequest {
        req_id: args.get("req-id").to_owned(),
        wire_bytes: 0,
        body,
        model: model.clone(),
        max_tokens,
    };
    let client = ProxyClient::new(NodeFactory { node: node.clone() });
    let events = client
        .call(lender, &req, signed.pubkey, Duration::from_secs(90))
        .await
        .unwrap_or_else(|e| fail_exit(5, &format!("CALL-FAIL: {e}")));

    let expect_reject = args.get("expect-reject");
    match events.last() {
        Some(ProxyEvent::Rejected { code, message, receipt }) => {
            let want = matches!((expect_reject, code), ("NotAllowlisted", ErrorCode::NotAllowlisted));
            if !want {
                fail_exit(5, &format!("非预期拒绝: {code:?} {message}"));
            }
            if receipt.is_some() {
                fail_exit(5, "拒绝路径不应携带结算收据");
            }
            println!("REJECT-OK code={code:?} msg={message}");
        }
        Some(ProxyEvent::Finished { receipt, stream_broken }) => {
            if !expect_reject.is_empty() {
                fail_exit(5, "期望拒绝却收到正常终结");
            }
            let expect_broken = args.has("expect-broken");
            if *stream_broken != expect_broken {
                fail_exit(5, &format!("断流标记不符: {stream_broken}"));
            }
            let total = (receipt.usage.input + receipt.usage.output) as i64;
            let mut ledger = Ledger::default();
            let applied = ledger
                .apply(receipt, &signed.pubkey)
                .unwrap_or_else(|e| fail_exit(5, &format!("收据入账失败: {e}")));
            if ledger.net(&receipt.borrower, &receipt.period) != -total {
                fail_exit(5, "借方净差与 usage 不符");
            }
            write_json(args.get("receipt-out"), receipt);
            write_ledger_file(args.get("ledger-out"), receipt);
            if *stream_broken {
                println!(
                    "STREAM-BROKEN estimated={} usage={}/{}",
                    receipt.estimated, receipt.usage.input, receipt.usage.output
                );
            }
            println!(
                "CALL-OK req_id={} sse_frames={} usage={}/{} net={}",
                receipt.req_id,
                events.iter().filter(|e| matches!(e, ProxyEvent::Sse(_))).count(),
                receipt.usage.input,
                receipt.usage.output,
                -total
            );
            println!("RECEIPT-VERIFY=PASS LEDGER-APPLY={applied}");
        }
        other => fail_exit(5, &format!("意外事件序列: {other:?}")),
    }
    node.shutdown();
}

fn write_json(path: &str, value: &impl serde::Serialize) {
    if path.is_empty() {
        return;
    }
    let text = serde_json::to_string_pretty(value).expect("serialize");
    let path = std::path::Path::new(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| fail_exit(5, &format!("mkdir {parent:?}: {e}")));
    }
    std::fs::write(path, text).unwrap_or_else(|e| fail_exit(5, &format!("write {}: {e}", path.display())));
}

/// 产品 ledger.json 形态（{v:1, receipts:[...]}），供 p2pctl llm-share ledger 直接读取。
fn write_ledger_file(path: &str, receipt: &Receipt) {
    write_json(path, &serde_json::json!({ "v": 1, "receipts": [receipt] }));
}

RUST_EOF
}


LOCAL_IP="$(ipconfig getifaddr en0 2>/dev/null || ipconfig getifaddr en1 2>/dev/null || echo 192.168.0.15)"
CTL="${ROOT}/apps/cli/target/debug/p2pctl"
HARNESS_BIN="${HARNESS_DIR}/target/debug/llm-smoke-harness"

build_local() {
    log "---- S1 进度: 本地 harness 生成与构建（路径依赖 ${ROOT}/crates，首次数分钟属预期）"
    gen_harness "${HARNESS_DIR}" "${ROOT}/crates"
    gt 900 cargo build --quiet --manifest-path "${HARNESS_DIR}/Cargo.toml" >&2 || die "本地 harness 构建失败"
    [ -x "${HARNESS_BIN}" ] || die "本地 harness 二进制缺失"
    if [ ! -x "${CTL}" ]; then
        log "---- S1 进度: 本地 p2pctl 缺失，补建"
        gt 900 cargo build --quiet --manifest-path "${ROOT}/apps/cli/Cargo.toml" >&2 || die "本地 p2pctl 构建失败"
    fi
    [ -x "${CTL}" ] || die "本地 p2pctl 二进制缺失"
}

supply_remote() {
    log "---- S1 进度: 探测 ${REMOTE_IP} rust 工具链（缺失则 rustup 装至用户家目录，禁 sudo）"
    local cargo_state
    cargo_state="$(remote_sh 'command -v cargo >/dev/null 2>&1 && echo CARGO-OK || ([ -x "$HOME/.cargo/bin/cargo" ] && echo CARGO-OK-HOME || echo CARGO-MISSING)')"
    case "${cargo_state}" in
        CARGO-OK*) log "---- S1 进度: 远端 cargo 已就绪" ;;
        CARGO-MISSING)
            log "---- S1 进度: 远端无 cargo，rustup 安装至用户家目录（可能数分钟）"
            gt 600 ssh -o BatchMode=yes -o ConnectTimeout=10 "${REMOTE}" \
                'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain stable --profile minimal' \
                >&2 || die "远端 rustup 安装失败"
            ;;
        *) die "远端工具链探测异常: ${cargo_state}" ;;
    esac

    log "---- S1 进度: git bundle 打包本仓库 HEAD 并供给远端（clone/pull 到 ~/llm-smoke-work/src）"
    mkdir -p "${WORK}"
    local bundle="${WORK}/p2p-smoke.bundle"
    gt 120 git -C "${ROOT}" bundle create "${bundle}" HEAD >&2 || die "bundle 创建失败"
    remote_sh "mkdir -p ${REMOTE_WORK}" || die "远端工作目录创建失败"
    gt 300 scp -o BatchMode=yes -o ConnectTimeout=10 "${bundle}" "${REMOTE}:${REMOTE_WORK}/src.bundle" >&2 || die "bundle 传输失败"
    if remote_sh "[ -d ${REMOTE_WORK}/src/.git ]"; then
        log "---- S1 进度: 远端已有 src，增量 fetch + reset（幂等更新）"
        remote_sh "cd ${REMOTE_WORK}/src && git fetch --quiet ../src.bundle HEAD && git reset --quiet --hard FETCH_HEAD" || die "远端代码更新失败"
    else
        log "---- S1 进度: 远端首次 clone（bundle 来源，无需公网 git 访问）"
        remote_sh "cd ${REMOTE_WORK} && git clone --quiet src.bundle src" || die "远端 clone 失败"
    fi


    log "---- S1 进度: 生成并上传 harness 源到远端（路径依赖远端 clone 内 crates）"
    gen_harness "${WORK}/harness-upload" "../src/crates"
    gt 120 tar -C "${WORK}/harness-upload" -cf - Cargo.toml src \
        | gt 120 ssh -o BatchMode=yes -o ConnectTimeout=10 "${REMOTE}" \
            "mkdir -p ${REMOTE_WORK}/harness && tar -C ${REMOTE_WORK}/harness -xf -" \
        || die "harness 上传失败"
    log "---- S1 进度: 远端构建 harness（首次含依赖编译，数分钟属预期，耐心等）"
    remote_sh "export PATH=\"\$HOME/.cargo/bin:\$PATH\"; cd ${REMOTE_WORK} && timeout 1200 cargo build --quiet --manifest-path harness/Cargo.toml" >&2 || die "远端 harness 构建失败"
    remote_sh "test -x ${REMOTE_WORK}/harness/target/debug/llm-smoke-harness" || die "远端 harness 二进制缺失"

    log "---- S1 进度: 远端构建 p2pctl（产品 CLI，供 S3 声明发布）"
    if ! remote_sh "test -x ${REMOTE_WORK}/src/apps/cli/target/debug/p2pctl"; then
        remote_sh "export PATH=\"\$HOME/.cargo/bin:\$PATH\"; cd ${REMOTE_WORK}/src && timeout 1200 cargo build --quiet --manifest-path apps/cli/Cargo.toml" >&2 || die "远端 p2pctl 构建失败"
    fi
    remote_sh "test -x ${REMOTE_WORK}/src/apps/cli/target/debug/p2pctl" || die "远端 p2pctl 二进制缺失"
    REMOTE_SUPPLIED=1
}

s1_supply() {
    build_local
    supply_remote
    sc_pass "S1" "供给：本端/远端工具链+代码+harness+p2pctl 全部就绪"
}

# ---- 拓扑：本地 bootstrap（rendezvous+观测反射）+ 102 出借方 ----

start_bootstrap() {
    log "---- S2 进度: 本地拉起 bootstrap（rendezvous 注册/查号 + 观测反射口）"
    nohup "$HARNESS_BIN" bootstrap --data "$WORK/boot" --quic "$BOOT_QUIC" \
        > "$WORK/boot.log" 2>&1 &
    LOCAL_PIDS+=("$!")
    local i
    for i in $(seq 1 40); do
        grep -q BOOTSTRAP-READY "$WORK/boot.log" 2>/dev/null && return 0
        sleep 0.5
    done
    die "bootstrap 启动超时（$WORK/boot.log）"
}

mint_borrowers() {
    gt 30 "$HARNESS_BIN" identity --data "$WORK/borrower/p2p-data" > "$WORK/borrower.peer"
    gt 30 "$HARNESS_BIN" identity --data "$WORK/borrower2/p2p-data" > "$WORK/borrower2.peer"
    gt 30 "$HARNESS_BIN" identity --data "$WORK/borrower-s4/p2p-data" > "$WORK/borrower-s4.peer"
    gt 30 "$HARNESS_BIN" identity --data "$WORK/borrower-s6b/p2p-data" > "$WORK/borrower-s6b.peer"
    BORR_PEER="$(sed -n 's/^peerId=//p' "$WORK/borrower.peer")"
    BORR2_PEER="$(sed -n 's/^peerId=//p' "$WORK/borrower2.peer")"
    BORR_S4_PEER="$(sed -n 's/^peerId=//p' "$WORK/borrower-s4.peer")"
    BORR_S6B_PEER="$(sed -n 's/^peerId=//p' "$WORK/borrower-s6b.peer")"
    [ -n "$BORR_PEER" ] && [ -n "$BORR2_PEER" ] && [ -n "$BORR_S4_PEER" ] && [ -n "$BORR_S6B_PEER" ] || die "借方身份生成失败"
}

wait_remote_ready() {
    local i
    for i in $(seq 1 60); do
        if remote_sh "grep -q '${2}' ${REMOTE_WORK}/logs/${1} 2>/dev/null"; then return 0; fi
        if remote_sh "grep -q 'HARNESS-FAIL' ${REMOTE_WORK}/logs/${1} 2>/dev/null"; then
            die "远端 ${1} 启动失败（见远端日志）"
        fi
        sleep 1
    done
    die "远端 ${1} 就绪超时"
}

start_lender() {
    log "---- S3 进度: 102 侧供给身份并签名发布能力声明（p2pctl offer publish，产品命令）"
    remote_sh "cd ${REMOTE_WORK} && rm -rf lender logs run && mkdir -p lender/p2p-data logs run \
        && export PATH=\"\$HOME/.cargo/bin:\$PATH\" \
        && ./harness/target/debug/llm-smoke-harness identity --data lender/p2p-data \
        && ./src/apps/cli/target/debug/p2pctl llm-share offer publish \
            --model ${MODEL} --spare ${MODEL}=1000000 \
            --period-ends ${PERIOD_ENDS} --data-dir lender > logs/publish.log 2>&1" \
        || die "远端身份供给/声明发布失败"
    remote_sh "if [ -f ${REMOTE_WORK}/run/serve.pid ] && kill -0 \"\$(cat ${REMOTE_WORK}/run/serve.pid)\" 2>/dev/null; then kill \"\$(cat ${REMOTE_WORK}/run/serve.pid)\" || true; sleep 1; fi; \
        opid=\$(ss -lnup 2>/dev/null | grep ':${LEND_QUIC} ' | grep -oE 'pid=[0-9]+' | head -1 | cut -d= -f2); \
        if [ -n \"\${opid}\" ] && grep -q 'llm-smoke-harne' \"/proc/\${opid}/comm\" 2>/dev/null; then kill \"\${opid}\" || true; sleep 1; fi" \
        || true
    remote_sh "cd ${REMOTE_WORK} && { RUST_LOG=warn nohup ./harness/target/debug/llm-smoke-harness serve \
            --data lender/p2p-data --quic ${LEND_QUIC} \
            --bootstrap ${LOCAL_IP}/u${BOOT_QUIC} --observation ${LOCAL_IP}:${OBS_PORT} \
            --allow "${BORR_PEER},${BORR_S4_PEER},${BORR_S6B_PEER}" --model ${MODEL} --period ${PERIOD} \
            --upstream-log logs/upstream.jsonl --net-limit 1000000 --broken-on 2 \
            --offer-file lender/llm-share/offer.json </dev/null > logs/serve.log 2>&1 </dev/null & echo \$! > run/serve.pid; }" \
        || die "远端出借方启动失败"
    wait_remote_ready "serve.log" "SERVE-READY"
    LENDER_PEER="$(remote_sh "sed -n 's/^peer=//p' ${REMOTE_WORK}/logs/publish.log | head -1")"
    [ -n "$LENDER_PEER" ] || die "未取得 102 出借方 PeerId"
    log "---- 拓扑就绪: lender(${LENDER_PEER}) @ ${REMOTE_IP}/u${LEND_QUIC}，borrower(${BORR_PEER}) 本机"
}

upstream_calls() {
    remote_sh "wc -l < ${REMOTE_WORK}/logs/upstream.jsonl 2>/dev/null || echo 0" 2>/dev/null | tr -d "[:space:]"
}

remote_ledger_receipts() {
    remote_sh "grep -o 'receipts=[0-9]*' ${REMOTE_WORK}/logs/serve.log | tail -1" 2>/dev/null | tr -d "[:space:]"
}

s2_s3_probe() {
    log "---- S2/S3: 借方经 rendezvous 发现 102 出借方并取声明选路（--probe）"
    local rc=0
    PROBE_OUT="$(gt 120 "$HARNESS_BIN" call --probe \
        --data "$WORK/borrower/p2p-data" \
        --bootstrap "$LOCAL_IP/u$BOOT_QUIC" --observation "$LOCAL_IP:$OBS_PORT" \
        --lender "$LENDER_PEER" --model "$MODEL" --discover-wait 30 2>&1)" || rc=$?
    printf '%s\n' "$PROBE_OUT"
    if [ "$rc" -ne 0 ]; then
        log "---- 诊断: 出借方 serve.log 尾部"
        remote_sh "grep -E 'WARN|ERROR|HARNESS-FAIL|SERVE-READY' ${REMOTE_WORK}/logs/serve.log 2>/dev/null | tail -6" || true
        die "probe 失败（rc=$rc)"
    fi
    printf '%s\n' "$PROBE_OUT" | grep -q "DISCOVER-OK" || sc_fail "S2" "rendezvous 查号未命中"
    printf '%s\n' "$PROBE_OUT" | grep -q "CONNECT-OK" || sc_fail "S2" "跨机连接未建立"
    sc_pass "S2" "互联：rendezvous 发现 + 跨机连接建立"
    printf '%s\n' "$PROBE_OUT" | grep -q "OFFER-OK" || sc_fail "S3" "声明验签/选路失败"
    PUBKEY="$(printf '%s\n' "$PROBE_OUT" | sed -n 's/^OFFER-OK pubkey=//p' | cut -d' ' -f1)"
    [ -n "$PUBKEY" ] || sc_fail "S3" "未取得声明公钥"
    sc_pass "S3" "出借方=102：声明签名发布 + 借方验签（Ed25519+TTL）+ 选路成功"
}

s4_call() {
    log "---- S4: 跨机真实流式 chat（上游=102 侧进程内 mock），双边记账 + 收据验签"
    local rc=0
    CALL_OUT="$(gt 150 "$HARNESS_BIN" call \
        --data "$WORK/borrower-s4/p2p-data" \
        --bootstrap "$LOCAL_IP/u$BOOT_QUIC" --observation "$LOCAL_IP:$OBS_PORT" \
        --lender "$LENDER_PEER" --model "$MODEL" --req-id req-t23-s4 --max-tokens 64 --discover-wait 60 \
        --receipt-out "$WORK/receipt.json" --ledger-out "$WORK/llm-share/ledger.json" 2>&1)" || rc=$?
    printf '%s\n' "$CALL_OUT"
    [ "$rc" -eq 0 ] || die "S4 调用失败（rc=$rc)"
    printf '%s\n' "$CALL_OUT" | grep -q "RECEIPT-VERIFY=PASS" || sc_fail "S4" "harness 收据验签失败"
    "$CTL" llm-share receipt verify "$WORK/receipt.json" --pubkey "$PUBKEY" | grep -q "verdict=PASS" \
        || sc_fail "S4" "产品 CLI 收据验签失败"
    "$CTL" llm-share ledger list --data-dir "$WORK" --period "$PERIOD" | grep -q "共 1 条流水" \
        || sc_fail "S4" "借方账本应恰一笔流水"
    [ "$(remote_ledger_receipts)" = "receipts=1" ] || sc_fail "S4" "出借方账本应恰一笔"
    [ "$(upstream_calls)" = "1" ] || sc_fail "S4" "上游应恰一次调用"
    sc_pass "S4" "跨机调用：流式回包 + 双边各记一笔 + 收据 Ed25519 验签 PASS（产品 CLI）"
}

s5_negative() {
    log "---- S5: 非 allowlist peer 请求须结构化拒绝且上游零调用"
    local u0 u1 rc=0
    u0="$(upstream_calls)"
    REJECT_OUT="$(gt 150 "$HARNESS_BIN" call \
        --data "$WORK/borrower2/p2p-data" \
        --bootstrap "$LOCAL_IP/u$BOOT_QUIC" --observation "$LOCAL_IP:$OBS_PORT" \
        --lender "$LENDER_PEER" --model "$MODEL" --req-id req-t23-s5 --discover-wait 60 \
        --expect-reject NotAllowlisted 2>&1)" || rc=$?
    printf '%s\n' "$REJECT_OUT"
    [ "$rc" -eq 0 ] || sc_fail "S5" "非 allowlist 调用未按预期被拒（rc=$rc)"
    printf '%s\n' "$REJECT_OUT" | grep -q "REJECT-OK code=NotAllowlisted" || sc_fail "S5" "未收到结构化 NotAllowlisted 拒绝"
    u1="$(upstream_calls)"
    [ "$u1" = "$u0" ] || sc_fail "S5" "上游调用计数增量非零（$u0 -> $u1）"
    [ "$(remote_ledger_receipts)" = "receipts=1" ] || sc_fail "S5" "拒绝路径不应产生新流水"
    sc_pass "S5" "负向：NotAllowlisted 结构化拒绝，上游调用增量为零"
}

s6_observable() {
    log "---- S6a: 坏节点路径（未注册 PeerId）须显式报错不挂死"
    local bad_peer rc=0
    bad_peer="$(gt 30 "$HARNESS_BIN" gen-peer | sed -n 's/^peerId=//p')"
    BAD_OUT="$(gt 60 "$HARNESS_BIN" call --probe \
        --data "$WORK/borrower/p2p-data" \
        --bootstrap "$LOCAL_IP/u$BOOT_QUIC" \
        --lender "$bad_peer" --model "$MODEL" --discover-wait 5 2>&1)" && rc=1 || rc=$?
    printf '%s\n' "$BAD_OUT" | tail -1
    [ "$rc" -ne 0 ] || sc_fail "S6" "坏节点路径竟然成功"
    printf '%s\n' "$BAD_OUT" | grep -q "DISCOVER-FAIL" || sc_fail "S6" "坏节点路径缺显式错误输出"

    log "---- S6b: 断流路径（第 2 次上游调用按剧本切断）须显式 STREAM-BROKEN 不挂死"
    local rc2=0
    BROKEN_OUT="$(gt 150 "$HARNESS_BIN" call \
        --data "$WORK/borrower-s6b/p2p-data" \
        --bootstrap "$LOCAL_IP/u$BOOT_QUIC" --observation "$LOCAL_IP:$OBS_PORT" \
        --lender "$LENDER_PEER" --model "$MODEL" --req-id req-t23-s6b --expect-broken --discover-wait 60 \
        --receipt-out "$WORK/receipt-broken.json" 2>&1)" || rc2=$?
    printf '%s\n' "$BROKEN_OUT"
    [ "$rc2" -eq 0 ] || sc_fail "S6" "断流路径失败（rc=$rc2)"
    printf '%s\n' "$BROKEN_OUT" | grep -q "STREAM-BROKEN" || sc_fail "S6" "断流未显式呈现"
    grep -q '"estimated": true' "$WORK/receipt-broken.json" || sc_fail "S6" "断流收据应为 estimated"
    "$CTL" llm-share receipt verify "$WORK/receipt-broken.json" --pubkey "$PUBKEY" | grep -q "verdict=PASS" \
        || sc_fail "S6" "断流 estimated 收据验签失败"
    sc_pass "S6" "可观察：坏节点/断流路径显式报错不挂死，estimated 收据可验签"
}

summary() {
    local s
    for s in S1 S2 S3 S4 S5 S6; do
        case " ${PASS_LIST[*]} " in
            *" ${s} "*) ;;
            *) sc_fail "${s}" "场景缺失" ;;
        esac
    done
    log "---- 全部场景: ${PASS_LIST[*]}"
}

main() {
    log "== T23 llm-share 真两机冒烟开始 ${REMOTE_IP} $(date -u +%FT%TZ) =="
    mkdir -p "$WORK"
    s1_supply
    start_bootstrap
    mint_borrowers
    start_lender
    s2_s3_probe
    s4_call
    s5_negative
    s6_observable
    summary
    log "SMOKE-OK"
}

main "$@"

