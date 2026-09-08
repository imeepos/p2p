//! 分享链接 E2E 服务端夹具（镜像 W3 装配）：
//! - RedeemFixture：/llm-share/redeem/1，ShareLedger 试激活 + allowlist 落条目
//!   （source=share:<id>）+ 台账持久化；拒绝码原样回帧，不外泄 share 是否存在。
//! - ShareProxyHandler：/llm-share/proxy/1，文件 allowlist 动态闸（默认拒绝，
//!   revoke 级联后即拒），命中按认证 PeerId 取专属 LenderProxy 记账。
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use llm_share_link::ledger::ShareLedger;
use llm_share_link::redeem::PROTOCOL_ID as REDEEM_PROTOCOL;
use llm_share_link::redeem::{RedeemCode, RedeemRequest, RedeemResponse};
use llm_share_proxy::server::LenderProxy;
use llm_share_proxy::ModelRoute;
use llm_share_proxy::{ErrorCode, ProxyConfig, ProxyFrame, PROTOCOL_ID};
use p2p::{BoxedStream, PeerId, ProtocolHandler, ProtocolId};
use p2p_cli::llm_share::allowlist;
use p2p_cli::llm_share::now_secs;
use p2p_cli::llm_share::rfc3339_now;
use p2p_identity::Keypair;
use p2p_protocol::{read_frame, write_chunked, write_frame};
use tokio::sync::Mutex;

pub type RouteFactory = Arc<dyn Fn() -> HashMap<String, ModelRoute> + Send + Sync>;

/// shares.json 落点（<data-dir>/llm-share/ 约定，与 p2p-cli 同源）。
pub fn shares_file(data_dir: &str) -> PathBuf {
    PathBuf::from(data_dir)
        .join("llm-share")
        .join(llm_share_link::ledger::FILE_NAME)
}

/// 兑换 handler：激活临界区互斥；写序 allowlist 先、台账持久化后（W3 同源，
/// 进程中断时多授可幂等收敛）。
pub struct RedeemFixture {
    data_dir: String,
    lock: Mutex<()>,
}

impl RedeemFixture {
    pub fn new(data_dir: &str) -> Self {
        Self {
            data_dir: data_dir.to_owned(),
            lock: Mutex::new(()),
        }
    }

    async fn activate(&self, peer: &str, token: &str) -> RedeemResponse {
        let _guard = self.lock.lock().await;
        let file = shares_file(&self.data_dir);
        let mut ledger = match ShareLedger::load_or_empty(&file) {
            Ok(ledger) => ledger,
            Err(e) => {
                eprintln!("b4: share 台账读取失败: {e}");
                return RedeemResponse::deny(RedeemCode::Invalid);
            }
        };
        let granted = match ledger.redeem(token, peer, now_secs()) {
            Ok(granted) => granted,
            Err(e) => {
                eprintln!("b4: 兑换拒绝 peer={peer} code={}", e.wire_code());
                return RedeemResponse::deny(RedeemCode::from(e));
            }
        };
        if let Err(e) = allowlist::allow(
            &self.data_dir,
            peer,
            &granted.models,
            None,
            Some(&granted.source),
            Some(granted.expires_at),
            &rfc3339_now(),
        ) {
            eprintln!("b4: allowlist 落条目失败（share 未消耗）: {e}");
            return RedeemResponse::deny(RedeemCode::Invalid);
        }
        if let Err(e) = ledger.save(&file) {
            eprintln!("b4: 台账持久化失败（allowlist 已落，重试幂等收敛）: {e}");
            return RedeemResponse::deny(RedeemCode::Invalid);
        }
        eprintln!("b4: share-redeemed peer={peer} source={}", granted.source);
        RedeemResponse::ok()
    }
}

#[async_trait]
impl ProtocolHandler for RedeemFixture {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(REDEEM_PROTOCOL).expect("redeem protocol id")
    }

    async fn handle_inbound(&self, peer: PeerId, mut stream: BoxedStream) -> std::io::Result<()> {
        let raw = read_frame(&mut stream).await?;
        let req: RedeemRequest = serde_json::from_slice(&raw)?;
        let reply = self.activate(&peer.to_string(), &req.token).await;
        write_frame(&mut stream, &serde_json::to_vec(&reply)?).await
    }

    /// 裸流无认证身份上下文：fail-closed。
    async fn handle(&self, _stream: BoxedStream) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "bare stream has no authenticated borrower",
        ))
    }
}

/// 代理 handler：文件 allowlist 动态闸 + per-peer LenderProxy（allowlist={peer}）；
/// 路由以工厂注入（每 peer 独立实例，ModelRoute 非 Clone 不做整表克隆）。
pub struct ShareProxyHandler {
    data_dir: String,
    routes: RouteFactory,
    period: String,
    net_limit: u64,
    keypair: Keypair,
    proxies: Mutex<HashMap<String, Arc<LenderProxy>>>,
}

impl ShareProxyHandler {
    pub fn new(
        data_dir: &str,
        routes: RouteFactory,
        period: &str,
        net_limit: u64,
        keypair: Keypair,
    ) -> Self {
        Self {
            data_dir: data_dir.to_owned(),
            routes,
            period: period.to_owned(),
            net_limit,
            keypair,
            proxies: Mutex::new(HashMap::new()),
        }
    }

    async fn proxy_for(&self, peer: &str) -> Arc<LenderProxy> {
        let keypair = self.keypair.clone();
        let routes = (self.routes)();
        let period = self.period.clone();
        let net_limit = self.net_limit;
        let lender_id = keypair.peer_id().to_string();
        self.proxies
            .lock()
            .await
            .entry(peer.to_owned())
            .or_insert_with(|| {
                let cfg = ProxyConfig {
                    lender_id,
                    period,
                    net_limit,
                    max_concurrent: 4,
                    allowlist: [peer.to_owned()].into_iter().collect(),
                    models: routes,
                };
                Arc::new(LenderProxy::new(cfg, keypair))
            })
            .clone()
    }
}

#[async_trait]
impl ProtocolHandler for ShareProxyHandler {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(PROTOCOL_ID).expect("proxy protocol id")
    }

    async fn handle_inbound(&self, peer: PeerId, mut stream: BoxedStream) -> std::io::Result<()> {
        let peer_id = peer.to_string();
        let file = allowlist::path(&self.data_dir);
        let list = allowlist::load_or_empty(&file)
            .map_err(|e| std::io::Error::other(format!("allowlist 读取失败: {e}")))?;
        if !list.entries.contains_key(&peer_id) {
            eprintln!("b4: borrow 拒（allowlist 无条目）peer={peer_id}");
            let frame = ProxyFrame::Error {
                code: ErrorCode::NotAllowlisted,
                message: format!("peer {peer_id} not in allowlist"),
                receipt: None,
            };
            let bytes = serde_json::to_vec(&frame)?;
            return write_chunked(&mut stream, &bytes).await;
        }
        let proxy = self.proxy_for(&peer_id).await;
        proxy.serve(stream, peer).await
    }

    /// 裸流无认证身份上下文：fail-closed。
    async fn handle(&self, _stream: BoxedStream) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "bare stream has no authenticated borrower",
        ))
    }
}
