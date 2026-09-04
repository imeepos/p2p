//! T19 集成测试共用夹具：进程内 mock 上游 + LoopbackHub 流配对，全程不出网。

// 各测试二进制只用到夹具的子集，死代码告警按夹具惯例整档豁免。
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use futures::StreamExt;
use llm_share_proxy::upstream::{SseByteStream, Upstream, UpstreamCall, UpstreamFailure};
use llm_share_proxy::{LenderProxy, ModelRoute, ProxyClient, ProxyConfig};
use p2p_identity::{Keypair, PeerId};
use p2p_protocol::LoopbackHub;

pub const TIMEOUT: Duration = Duration::from_secs(10);

/// mock 上游剧本：逐 call 从后往前弹出。
pub enum Script {
    /// 依次吐出的 SSE 字节块（含 usage 块则按实收）。
    Canned(Vec<Vec<u8>>),
    /// 吐完前缀后流中断，无 usage（断流计费路径）。
    BrokenAfter(Vec<Vec<u8>>),
    /// 连接即被上游拒绝（429 等，不产生流水）。
    Rejected(u16),
    /// 挂起不吐（占住并发额度）。
    Stalled,
}

pub struct MockUpstream {
    calls: AtomicUsize,
    script: StdMutex<Vec<Script>>,
}

impl MockUpstream {
    pub fn new(script: Vec<Script>) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            script: StdMutex::new(script),
        })
    }

    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
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
            Some(Script::Rejected(status)) => Err(UpstreamFailure::Rejected(status)),
            Some(Script::Stalled) | None => Ok(futures::stream::pending().boxed()),
        }
    }
}

/// 服务端配置：白名单只挂 gpt-4o；base_url 指向 .invalid 域（绝不可出网）。
pub fn proxy_config(
    lender_id: &str,
    allowed: &[&Keypair],
    upstream: Arc<dyn Upstream>,
    net_limit: u64,
    max_concurrent: u32,
) -> ProxyConfig {
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
        net_limit,
        max_concurrent,
        allowlist: allowed.iter().map(|k| k.peer_id().to_string()).collect(),
        models,
    }
}

/// 起 服务端 + 客户端：inbound 循环以认证借方身份喂给 serve（模拟 swarm 认证后接线）。
pub async fn spin(
    lender: &Keypair,
    cfg: ProxyConfig,
    borrower: PeerId,
) -> (Arc<LenderProxy>, ProxyClient<LoopbackHub>) {
    let (hub, mut inbound) = LoopbackHub::new(64, 256 * 1024);
    let proxy = Arc::new(LenderProxy::new(cfg, lender.clone()));
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

pub fn proxy_request(req_id: &str, model: &str, max_tokens: u64) -> llm_share_proxy::ProxyRequest {
    let body = serde_json::json!({
        "req_id": req_id, "model": model, "max_tokens": max_tokens,
        "messages": [{ "role": "user", "content": "ping" }], "stream": true
    });
    let raw = serde_json::to_vec(&body).expect("json");
    llm_share_proxy::ProxyRequest::parse(&raw).expect("valid request")
}

pub fn sse_data(payload: &str) -> Vec<u8> {
    format!(
        "data: {payload}

"
    )
    .into_bytes()
}

pub fn usage_chunk(prompt: u64, completion: u64) -> Vec<u8> {
    sse_data(&format!(
        "{{\"usage\":{{\"prompt_tokens\":{prompt},\"completion_tokens\":{completion}}}}}"
    ))
}

/// 轮询直到条件成立（mock 状态异步可见），5s 超时即红。
pub async fn until(mut cond: impl FnMut() -> bool) {
    for _ in 0..500 {
        if cond() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("condition not reached within 5s");
}
