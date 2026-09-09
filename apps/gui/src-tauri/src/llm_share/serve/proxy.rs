//! /llm-share/proxy/1 出借方门禁 handler（契约 §16.6 #4，W3）：
//!【外层动态闸】共享 AllowlistGate admit 判定（含到期）→ 读请求帧 →
//! 重启幂等（ledger.json 重建 settled 索引 + 在途 pending）→ 全局并发闸；
//!【内层记账】按认证 PeerId 取专属 LenderProxy（闸 1 判定源已切 authz，见
//! super::authz；冻结/净差/收据索引按 peer 隔离，账期与净差上限同主配置），
//! 三闸与记账复用 W1 交付。入站身份一律取握手认证 PeerId（handle_inbound），
//! 帧内自报不信。每次结算后 drain 收据幂等 append 到 llm-share/ledger.json
//!（防双记账）。

use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use llm_share_ledger::Receipt;
use llm_share_proxy::server::ModelRoute;
use llm_share_proxy::{
    ErrorCode, Gate1Fn, LenderProxy, ProxyConfig, ProxyFrame, ProxyRequest, PROTOCOL_ID,
};
use p2p::{BoxedStream, PeerId, ProtocolHandler, ProtocolId};
use p2p_identity::Keypair;
use p2p_protocol::{read_chunked, write_chunked};
use tokio::sync::{Mutex as AsyncMutex, Semaphore};

use super::authz;
use super::gate::AllowlistGate;
use super::replay::{encode_replay_frames, rebuild_settled, PrefixStream};

pub struct ServeCore {
    pub(crate) data_dir: String,
    pub(crate) lender_id: String,
    pub(crate) period: String,
    pub(crate) net_limit: u64,
    pub(crate) max_concurrent: u32,
    pub(crate) keypair: Keypair,
    pub(crate) routes: HashMap<String, ModelRoute>,
    pub(crate) gate: Arc<AllowlistGate>,
    /// 闸 1 判定源（authz.check(llm.borrow)，装配见 super::authz）。
    pub(crate) gate1: Gate1Fn,
    /// 重启幂等索引：ledger.json 出借侧收据重建 + 结算后即时登记。
    pub(crate) settled: AsyncMutex<HashMap<String, Receipt>>,
    pub(crate) pending: AsyncMutex<HashSet<String>>,
    pub(crate) in_flight: Arc<Semaphore>,
    pub(crate) proxies: AsyncMutex<HashMap<String, Arc<LenderProxy>>>,
}

/// 装配入参快照（offer/providers 在 node_start 时刻定格，运行中不热更）。
pub(crate) struct ServeCoreConfig {
    pub data_dir: String,
    pub lender_id: String,
    pub period: String,
    pub net_limit: u64,
    pub max_concurrent: u32,
    pub keypair: Keypair,
    pub routes: HashMap<String, ModelRoute>,
    pub gate: Arc<AllowlistGate>,
}

impl ServeCore {
    pub(crate) fn new(config: ServeCoreConfig) -> Self {
        let ServeCoreConfig {
            data_dir,
            lender_id,
            period,
            net_limit,
            max_concurrent,
            keypair,
            routes,
            gate,
        } = config;
        let settled = rebuild_settled(&data_dir, &lender_id, &keypair);
        let gate1 = authz::gate1(&data_dir);
        let max_concurrent = max_concurrent.max(1);
        Self {
            data_dir,
            lender_id,
            period,
            net_limit,
            max_concurrent,
            keypair,
            routes,
            gate,
            gate1,
            settled: AsyncMutex::new(settled),
            pending: AsyncMutex::new(HashSet::new()),
            in_flight: Arc::new(Semaphore::new(max_concurrent as usize)),
            proxies: AsyncMutex::new(HashMap::new()),
        }
    }

    /// 认证 peer 的专属记账实例：闸 1 以 authz 绑定表裁决（cfg.allowlist 转
    /// 只读归档不再消费，回滚 = revert 装配提交恢复旧判定）。
    pub(crate) async fn proxy_for(self: &Arc<Self>, peer: &str) -> Arc<LenderProxy> {
        let mut proxies = self.proxies.lock().await;
        proxies
            .entry(peer.to_owned())
            .or_insert_with(|| {
                let cfg = ProxyConfig {
                    lender_id: self.lender_id.clone(),
                    period: self.period.clone(),
                    net_limit: self.net_limit,
                    max_concurrent: self.max_concurrent,
                    allowlist: HashSet::from([peer.to_owned()]),
                    models: clone_routes(&self.routes),
                };
                Arc::new(
                    LenderProxy::new(cfg, self.keypair.clone())
                        .with_gate1_authz(self.gate1.clone()),
                )
            })
            .clone()
    }

    /// 结算收据幂等落盘（record 按 req_id 去重 + 原子写），登记重建索引。
    pub(crate) async fn persist_receipts(&self, proxy: &LenderProxy) {
        for receipt in proxy.receipts().await {
            let mut settled = self.settled.lock().await;
            if settled.contains_key(&receipt.req_id) {
                continue;
            }
            match p2p_cli::llm_share::ledger::record(&self.data_dir, &receipt) {
                Ok(report) => {
                    settled.insert(receipt.req_id.clone(), receipt.clone());
                    tracing::info!(
                        req_id = %receipt.req_id,
                        appended = report.appended,
                        "llm-share 出借侧收据已持久化"
                    );
                }
                Err(e) => tracing::error!(
                    req_id = %receipt.req_id,
                    "llm-share 收据持久化失败（应答不阻塞，索引未登记待重试）: {e}"
                ),
            }
        }
    }
}

pub struct ProxyGateHandler {
    pub(crate) core: Arc<ServeCore>,
}

impl ProxyGateHandler {
    pub(crate) fn new(core: Arc<ServeCore>) -> Self {
        Self { core }
    }
}

#[async_trait]
impl ProtocolHandler for ProxyGateHandler {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(PROTOCOL_ID).expect("proxy protocol id")
    }

    async fn handle_inbound(&self, peer: PeerId, mut stream: BoxedStream) -> io::Result<()> {
        let peer_str = peer.to_string();
        if !self
            .core
            .gate
            .admits(&peer_str, p2p_cli::llm_share::now_secs())
        {
            tracing::warn!(peer = %peer_str, "llm-share proxy 拒绝：not_allowlisted（allowlist 无条目或已过期）");
            return write_error(
                &mut stream,
                ErrorCode::NotAllowlisted,
                "peer not in allowlist".into(),
                None,
            )
            .await;
        }
        let raw = read_chunked(&mut stream).await?;
        let parsed = ProxyRequest::parse(&raw);
        let mut inflight: Option<String> = None;
        if let Ok(req) = &parsed {
            let settled = self.core.settled.lock().await;
            if let Some(stale) = settled.get(&req.req_id) {
                let stale = stale.clone();
                drop(settled);
                tracing::warn!(peer = %peer_str, req_id = %req.req_id, "llm-share proxy 拒绝：req_id 重放（重建索引命中，防双记账）");
                return write_error(
                    &mut stream,
                    ErrorCode::DuplicateReqId,
                    format!("req_id {} already settled", req.req_id),
                    Some(stale),
                )
                .await;
            }
            drop(settled);
            let mut pending = self.core.pending.lock().await;
            if !pending.insert(req.req_id.clone()) {
                drop(pending);
                return write_error(
                    &mut stream,
                    ErrorCode::DuplicateReqId,
                    format!("req_id {} in flight", req.req_id),
                    None,
                )
                .await;
            }
            inflight = Some(req.req_id.clone());
        }
        let permit = match self.core.in_flight.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                release(&self.core, inflight).await;
                return write_error(
                    &mut stream,
                    ErrorCode::ConcurrencyExceeded,
                    "concurrency limit".into(),
                    None,
                )
                .await;
            }
        };
        let proxy = self.core.proxy_for(&peer_str).await;
        let prefix = match encode_replay_frames(&raw).await {
            Ok(prefix) => prefix,
            Err(e) => {
                release(&self.core, inflight).await;
                return Err(e);
            }
        };
        let result = proxy
            .serve(Box::new(PrefixStream::new(prefix, stream)), peer)
            .await;
        self.core.persist_receipts(&proxy).await;
        release(&self.core, inflight).await;
        drop(permit);
        result
    }

    async fn handle(&self, stream: BoxedStream) -> io::Result<()> {
        self.handle_inbound(PeerId::from_bytes([0u8; 32]), stream)
            .await
    }
}

async fn release(core: &Arc<ServeCore>, inflight: Option<String>) {
    if let Some(req_id) = inflight {
        core.pending.lock().await.remove(&req_id);
    }
}

async fn write_error(
    stream: &mut BoxedStream,
    code: ErrorCode,
    message: String,
    receipt: Option<Receipt>,
) -> io::Result<()> {
    let frame = ProxyFrame::Error {
        code,
        message,
        receipt,
    };
    let bytes = serde_json::to_vec(&frame)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    write_chunked(stream, &bytes).await
}

/// ModelRoute 无 Clone（含上游 trait 对象），按字段手工复制共享上游实例。
pub(crate) fn clone_routes(routes: &HashMap<String, ModelRoute>) -> HashMap<String, ModelRoute> {
    routes
        .iter()
        .map(|(model, route)| {
            (
                model.clone(),
                ModelRoute {
                    base_url: route.base_url.clone(),
                    api_key: route.api_key.clone(),
                    upstream: route.upstream.clone(),
                },
            )
        })
        .collect()
}
