//! 出借方代理服务端状态与三闸准入（§4 步骤 2/3、§7.1）：
//! allowlist -> 模型白名单 -> req_id 幂等 -> 并发 -> 预授权冻结。
//! 任一闸拒绝返回可区分错误码：上游零调用、流水零产生。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use llm_share_ledger::{FreezeRequest, HoldManager, Ledger, Receipt};
use p2p_identity::{Keypair, PeerId};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

use crate::error::ErrorCode;
use crate::sse::estimate_tokens;
use crate::upstream::Upstream;
use crate::wire::ProxyRequest;

/// 单模型上游路由：key 仅存出借方进程内存（§6）。
pub struct ModelRoute {
    pub base_url: String,
    pub api_key: String,
    pub upstream: Arc<dyn Upstream>,
}

/// 服务端装配配置：allowlist/白名单/限额由装配方（apps 层）给定。
pub struct ProxyConfig {
    /// 本机 PeerId base58（流水 lender 侧标识）。
    pub lender_id: String,
    /// 账期（出借方本地口径，Q1）。
    pub period: String,
    /// 单借方净差上限（记账 token）；装配方按 LimitPolicy 折算声明 spare 得出。
    pub net_limit: u64,
    pub max_concurrent: u32,
    /// 借方 PeerId base58 白名单。
    pub allowlist: HashSet<String>,
    pub models: HashMap<String, ModelRoute>,
}

/// req_id 状态索引：settled 承载重放回传，pending 挡在途重放（MVP A4）。
#[derive(Default)]
pub(crate) struct IdempotencyIndex {
    pub settled: HashMap<String, Receipt>,
    pub pending: HashSet<String>,
}

/// 三闸拒绝材料：错误码 + 可述原因 + 重放时附原收据。
pub(crate) struct GateReject {
    pub code: ErrorCode,
    pub message: String,
    pub receipt: Option<Receipt>,
}

/// 通过全部闸后的就绪材料：上游路由与并发许可（permit 随请求生命周期 drop 释放）。
pub(crate) struct Admitted<'a> {
    pub route: &'a ModelRoute,
    pub permit: OwnedSemaphorePermit,
}

pub struct LenderProxy {
    pub(crate) cfg: ProxyConfig,
    pub(crate) keypair: Keypair,
    pub(crate) holds: Mutex<HoldManager>,
    pub(crate) ledger: Mutex<Ledger>,
    pub(crate) index: Mutex<IdempotencyIndex>,
    in_flight: Arc<Semaphore>,
}

impl LenderProxy {
    pub fn new(cfg: ProxyConfig, keypair: Keypair) -> Self {
        let in_flight = Arc::new(Semaphore::new(cfg.max_concurrent.max(1) as usize));
        Self {
            cfg,
            keypair,
            holds: Mutex::new(HoldManager::default()),
            ledger: Mutex::new(Ledger::default()),
            index: Mutex::new(IdempotencyIndex::default()),
            in_flight,
        }
    }

    pub fn lender_id(&self) -> &str {
        &self.cfg.lender_id
    }

    pub fn lender_pubkey(&self) -> [u8; 32] {
        self.keypair.public()
    }

    /// 本地账本快照：对账与观测用（§5.1 双方各持全量副本）。
    pub async fn ledger(&self) -> Ledger {
        self.ledger.lock().await.clone()
    }

    /// 已结算收据清单（req_id -> 收据），幂等审计用。
    pub async fn receipts(&self) -> Vec<Receipt> {
        self.index.lock().await.settled.values().cloned().collect()
    }

    /// 三闸准入。顺序：allowlist -> 模型 -> 幂等 -> 并发 -> 冻结；
    /// 廉价安全闸在前，冻结（需锁账本）最后，失败时并发许可自动随 GateReject 释放。
    pub(crate) async fn admit<'a>(
        &'a self,
        borrower: &PeerId,
        req: &ProxyRequest,
    ) -> Result<Admitted<'a>, Box<GateReject>> {
        let borrower_id = borrower.to_string();
        if !self.cfg.allowlist.contains(&borrower_id) {
            return Err(self.gate_reject(
                ErrorCode::NotAllowlisted,
                format!("peer {borrower_id} not in allowlist"),
                None,
            ));
        }
        let route = self.cfg.models.get(&req.model).ok_or_else(|| {
            self.gate_reject(
                ErrorCode::ModelNotServed,
                format!("model {} not served", req.model),
                None,
            )
        })?;
        {
            let mut index = self.index.lock().await;
            if let Some(receipt) = index.settled.get(&req.req_id) {
                let stale = receipt.clone();
                return Err(self.gate_reject(
                    ErrorCode::DuplicateReqId,
                    format!("req_id {} already settled", req.req_id),
                    Some(stale),
                ));
            }
            // 在途重放同挡：HoldManager 的 freeze 幂等只会放行，必须在此拦截（MVP A4）。
            if !index.pending.insert(req.req_id.clone()) {
                return Err(self.gate_reject(
                    ErrorCode::DuplicateReqId,
                    format!("req_id {} in flight", req.req_id),
                    None,
                ));
            }
        }
        let hold = self.hold_request(borrower_id, req);
        let permit = self.in_flight.clone().try_acquire_owned().map_err(|_| {
            self.gate_reject(
                ErrorCode::ConcurrencyExceeded,
                "concurrency limit".into(),
                None,
            )
        })?;
        self.freeze(hold).await?;
        Ok(Admitted { route, permit })
    }

    /// 预授权冻结硬闸（MVP A2）：est = 输入估算 + max_tokens；净差以借方欠额（取负入参）计。
    /// 冻结失败同步清 pending，req_id 状态不残留（重放可重试并得到同一冻结拒绝）。
    async fn freeze(&self, hold: FreezeRequest) -> Result<(), Box<GateReject>> {
        let req_id = hold.req_id.clone();
        let ledger_net = -self
            .ledger
            .lock()
            .await
            .net(&hold.borrower, &self.cfg.period);
        match self
            .holds
            .lock()
            .await
            .freeze(self.cfg.net_limit, ledger_net, hold)
        {
            Ok(()) => Ok(()),
            Err(e) => {
                self.index.lock().await.pending.remove(&req_id);
                Err(self.gate_reject(ErrorCode::FreezeInsufficient, format!("freeze: {e}"), None))
            }
        }
    }

    fn hold_request(&self, borrower_id: String, req: &ProxyRequest) -> FreezeRequest {
        FreezeRequest {
            req_id: req.req_id.clone(),
            lender: self.cfg.lender_id.clone(),
            borrower: borrower_id,
            period: self.cfg.period.clone(),
            est: estimate_tokens(req.wire_bytes) + req.max_tokens,
        }
    }

    fn gate_reject(
        &self,
        code: ErrorCode,
        message: String,
        receipt: Option<Receipt>,
    ) -> Box<GateReject> {
        tracing::warn!(lender = %self.cfg.lender_id, code = ?code, %message, "proxy request rejected");
        Box::new(GateReject {
            code,
            message,
            receipt,
        })
    }
}
