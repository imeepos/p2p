//! /llm-share/redeem/1 出借方 handler（契约 §16.6 #3，W3）：request-response
//! 一问一答（client 写单帧请求、读单帧应答）。入站身份取握手认证 PeerId
//!（handle_inbound），帧内自报不信。兑换激活互斥临界区：持 AllowlistGate
//! 写锁串行【share 台账试激活 → allowlist 落条目 → 台账持久化】（写序
//! allowlist 先、激活持久化后，进程中断时多授可幂等收敛）；失败包络统一
//! 回结构化拒绝码，不外泄 share 是否存在。按认证 PeerId 连续失败计数冷却。

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use llm_share_link::ledger::ShareLedger;
use llm_share_link::redeem::{RedeemCode, RedeemRequest, RedeemResponse, PROTOCOL_ID};
use p2p::{BoxedStream, PeerId, ProtocolHandler, ProtocolId};
use p2p_protocol::{read_frame, write_frame};
use tokio::sync::Mutex as AsyncMutex;

use super::gate::AllowlistGate;
use crate::llm_share::LlmShareStore;

/// 兑换连续失败冷却阈值与窗（按认证 PeerId，§5.4 限流/幂等）。
const COOLDOWN_THRESHOLD: u32 = 3;
const COOLDOWN_SECS: u64 = 60;

#[derive(Default)]
struct Throttle {
    failures: u32,
    blocked_until: Option<u64>,
}

pub struct RedeemHandler {
    pub(crate) store: LlmShareStore,
    pub(crate) gate: Arc<AllowlistGate>,
    throttle: AsyncMutex<HashMap<String, Throttle>>,
}

impl RedeemHandler {
    pub(crate) fn new(store: LlmShareStore, gate: Arc<AllowlistGate>) -> Self {
        Self {
            store,
            gate,
            throttle: AsyncMutex::new(HashMap::new()),
        }
    }

    async fn throttled(&self, peer: &str, now: u64) -> bool {
        let mut throttle = self.throttle.lock().await;
        let entry = throttle.entry(peer.to_owned()).or_default();
        matches!(entry.blocked_until, Some(until) if now < until)
    }

    async fn record_failure(&self, peer: &str, now: u64) {
        let mut throttle = self.throttle.lock().await;
        let entry = throttle.entry(peer.to_owned()).or_default();
        entry.failures = entry.failures.saturating_add(1);
        if entry.failures >= COOLDOWN_THRESHOLD {
            entry.blocked_until = Some(now + COOLDOWN_SECS);
            entry.failures = 0;
        }
    }

    async fn record_success(&self, peer: &str) {
        let mut throttle = self.throttle.lock().await;
        if let Some(entry) = throttle.get_mut(peer) {
            entry.failures = 0;
            entry.blocked_until = None;
        }
    }

    /// 兑换激活（互斥临界区）：试激活在台账克隆上进行，拒绝码全部原样回帧、
    /// 不落盘；成功路径按【allowlist 先、激活持久化后】写序提交。
    pub(crate) async fn activate(&self, peer: &str, token: &str, now: u64) -> RedeemResponse {
        let ledger_file = self.store.shares_file();
        self.gate.critical(|dir, state| {
            // 试激活直接在载入副本上进行：allowlist 写失败或台账保存失败时
            // 不落盘，share 不消耗（写序 allowlist 先、激活持久化后）。
            let mut ledger = match ShareLedger::load_or_empty(&ledger_file) {
                Ok(ledger) => ledger,
                Err(e) => {
                    tracing::error!(peer, "share-redeem 台账读取失败: {e}");
                    return RedeemResponse::deny(RedeemCode::Invalid);
                }
            };
            let result = match ledger.redeem(token, peer, now) {
                Ok(result) => result,
                Err(e) => {
                    tracing::info!(peer, code = e.wire_code(), "share-redeem 拒绝");
                    return RedeemResponse::deny(RedeemCode::from(e));
                }
            };
            let granted_at = p2p_cli::llm_share::rfc3339_now();
            if let Err(e) = AllowlistGate::write_allow_locked(
                dir,
                state,
                peer,
                &result.models,
                &result.source,
                result.expires_at,
                &granted_at,
            ) {
                tracing::error!(
                    peer,
                    "share-redeem allowlist 落条目失败（share 未消耗）: {e}"
                );
                return RedeemResponse::deny(RedeemCode::Invalid);
            }
            if let Err(e) = ledger.save(&ledger_file) {
                tracing::error!(
                    peer,
                    "share-redeem 台账持久化失败（allowlist 已落，重试幂等收敛）: {e}"
                );
                return RedeemResponse::deny(RedeemCode::Invalid);
            }
            tracing::info!(peer, source = %result.source, "share-redeemed");
            RedeemResponse::ok()
        })
    }
}

#[async_trait]
impl ProtocolHandler for RedeemHandler {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(PROTOCOL_ID).expect("redeem protocol id")
    }

    async fn handle_inbound(&self, peer: PeerId, mut stream: BoxedStream) -> std::io::Result<()> {
        let peer_str = peer.to_string();
        let now = p2p_cli::llm_share::now_secs();
        if self.throttled(&peer_str, now).await {
            tracing::warn!(peer = %peer_str, "share-redeem 冷却中（连续失败限流）");
            return write_reply(&mut stream, RedeemResponse::deny(RedeemCode::Invalid)).await;
        }
        let raw = match read_frame(&mut stream).await {
            Ok(raw) => raw,
            Err(e) => return Err(e),
        };
        let request: RedeemRequest = match serde_json::from_slice(&raw) {
            Ok(req) => req,
            Err(e) => {
                tracing::warn!(peer = %peer_str, "share-redeem 请求帧非法: {e}");
                self.record_failure(&peer_str, now).await;
                return write_reply(&mut stream, RedeemResponse::deny(RedeemCode::Invalid)).await;
            }
        };
        let reply = self.activate(&peer_str, &request.token, now).await;
        if reply.ok {
            self.record_success(&peer_str).await;
        } else {
            self.record_failure(&peer_str, now).await;
        }
        write_reply(&mut stream, reply).await
    }

    async fn handle(&self, stream: BoxedStream) -> std::io::Result<()> {
        self.handle_inbound(PeerId::from_bytes([0u8; 32]), stream)
            .await
    }
}

async fn write_reply(stream: &mut BoxedStream, reply: RedeemResponse) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(&reply)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    write_frame(stream, &bytes).await
}
