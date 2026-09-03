//! relay 会话命令入口（E8 自 mod.rs 迁出）：降级链 2/3 跳的统一通道。
//! 负载感知派发（T3）：候选按选择器排序，失败自动换下一候选（TURN
//! 客户端多候选回退实践）；无观测候选殿后，命令在会话队列排队等重连。

use std::io;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use p2p_identity::PeerId;
use p2p_relay::RelayHealth;
use p2p_transport::TransportAddr;
use tokio::sync::mpsc;

use super::relay_selector::{jitter_value, order_candidates, RelayCandidate};
use super::relay_session::RelayCmd;
use super::{Mux, Swarm};

/// 常驻会话句柄：命令入口 + 地址 + 健康快照槽（会话重连后换新句柄）。
#[derive(Clone)]
pub(super) struct RelaySessionHandle {
    pub(super) tx: mpsc::Sender<RelayCmd>,
    pub(super) addr: TransportAddr,
    pub(super) health: Arc<Mutex<Option<Arc<RelayHealth>>>>,
}

impl Swarm {
    /// 降级链 2/3 跳（打洞 + 中继电路）经由的会话命令入口。
    pub(super) async fn relay_degrade(&self, peer: PeerId) -> io::Result<Mux> {
        let handles = self.relay_sessions.lock().expect("relay lock").clone();
        if handles.is_empty() {
            tracing::debug!(%peer, "relay fallback unavailable: no relay configured");
            return Err(io::Error::other("no relay configured"));
        }
        let order = self.selector_order(&handles);
        let mut last: Option<io::Error> = None;
        for idx in order {
            let handle = &handles[idx];
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            if handle
                .tx
                .send(RelayCmd::Degrade {
                    peer,
                    reply: reply_tx,
                })
                .await
                .is_err()
            {
                last = Some(io::Error::other("relay session closed"));
                continue;
            }
            match reply_rx.await {
                Ok(Ok(mux)) => {
                    self.last_relay_idx.store(idx, Ordering::Relaxed);
                    return Ok(mux);
                }
                Ok(Err(e)) => {
                    tracing::warn!(%peer, relay = %handle.addr, error = %e,
                        "relay degrade failed; trying next candidate");
                    last = Some(e);
                }
                Err(_) => {
                    last = Some(io::Error::other("relay session dropped the request"));
                }
            }
        }
        Err(last.unwrap_or_else(|| io::Error::other("no relay session available")))
    }

    /// 选择器输入：健康快照 + 现任滞回 + 抖动打散；无观测候选按原序殿后。
    fn selector_order(&self, handles: &[RelaySessionHandle]) -> Vec<usize> {
        let cands: Vec<RelayCandidate> = handles
            .iter()
            .enumerate()
            .filter_map(|(i, h)| {
                let slot = h.health.lock().expect("health slot");
                slot.as_ref().map(|rh| RelayCandidate {
                    index: i,
                    health: rh.snapshot(),
                })
            })
            .collect();
        let cfg = &self.relay_selection_cfg;
        let last = self.last_relay_idx.load(Ordering::Relaxed);
        let current = (last < handles.len()).then_some(last);
        let mut order = order_candidates(&cands, current, cfg, || jitter_value(cfg.jitter_ms));
        let observed: std::collections::HashSet<usize> = order.iter().copied().collect();
        order.extend((0..handles.len()).filter(|i| !observed.contains(i)));
        order
    }

    pub(super) fn has_relay_sessions(&self) -> bool {
        !self.relay_sessions.lock().expect("relay lock").is_empty()
    }

    pub(super) fn add_relay_session(&self, handle: RelaySessionHandle) {
        self.relay_sessions.lock().expect("relay lock").push(handle);
    }
}
