//! 服务端记账（纯内存，无 IO）：链路配额、控制通道注册、电路登记与配对。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use p2p_mux::BoxedStream;
use tokio::io::WriteHalf;
use tokio::sync::Mutex as AsyncMutex;

use crate::messages::errcode;

/// 控制流写半（读半由控制循环独占）。
pub(crate) type CtrlWrite = AsyncMutex<WriteHalf<BoxedStream>>;

/// Reserve 缺省 TTL。
pub(crate) const DEFAULT_TTL_SECS: u64 = 300;
/// Reserve TTL 上限（防资源长占）。
pub(crate) const MAX_TTL_SECS: u64 = 3600;

/// 停在电路槽里等配对的第一条 Connect 流。
pub(crate) struct PendingStream {
    pub peer: String,
    pub stream: BoxedStream,
}

pub(crate) struct CircuitSlot {
    pub owner: String,
    pub expires: Instant,
    pub pending: Option<PendingStream>,
}

/// 一次 connect 的裁决结果。
pub(crate) enum CircuitOutcome {
    /// 第一条：流已收下等配对。
    Parked,
    /// 第二条：待配对流 + 本条流，可以桥接。
    Paired(PendingStream, BoxedStream),
    /// 拒绝码、信息与原流（写拒绝帧用）。
    Rejected(u32, String, BoxedStream),
}

/// 到期被清扫的电路；holder 是被丢弃的待配对流归属者。
pub(crate) struct ExpiredCircuit {
    pub cid: u64,
    pub holder: Option<String>,
}

pub(crate) struct RelayState {
    links: HashMap<String, usize>,
    circuit_load: HashMap<String, usize>,
    circuits: HashMap<u64, CircuitSlot>,
    controls: HashMap<String, Arc<CtrlWrite>>,
    next_circuit: u64,
}

impl RelayState {
    pub(crate) fn new() -> Self {
        Self {
            links: HashMap::new(),
            circuit_load: HashMap::new(),
            circuits: HashMap::new(),
            controls: HashMap::new(),
            next_circuit: 1,
        }
    }

    pub(crate) fn register_link(&mut self, peer: &str, max: usize) -> bool {
        let count = self.links.get(peer).copied().unwrap_or(0);
        if count >= max {
            return false;
        }
        self.links.insert(peer.to_string(), count + 1);
        true
    }

    pub(crate) fn unregister_link(&mut self, peer: &str) {
        if let Some(n) = self.links.get_mut(peer) {
            *n -= 1;
            if *n == 0 {
                self.links.remove(peer);
            }
        }
    }

    /// 后到覆盖：以该 Peer 最新的控制流为准。
    pub(crate) fn register_control(&mut self, peer: &str, half: Arc<CtrlWrite>) {
        self.controls.insert(peer.to_string(), half);
    }

    pub(crate) fn control_of(&self, peer: &str) -> Option<Arc<CtrlWrite>> {
        self.controls.get(peer).cloned()
    }

    /// 仅当登记项仍是这条写半时移除（不误删后注册的新控制流）。
    pub(crate) fn remove_control_if(&mut self, peer: &str, half: &Arc<CtrlWrite>) {
        if self.controls.get(peer).is_some_and(|h| Arc::ptr_eq(h, half)) {
            self.controls.remove(peer);
        }
    }

    /// 发放电路；超配额返回错误码。TTL 0 用缺省值，超过上限截断。
    pub(crate) fn issue_circuit(&mut self, owner: &str, ttl_secs: u64, max_per_peer: usize) -> Result<u64, u32> {
        let load = self.circuit_load.get(owner).copied().unwrap_or(0);
        if load >= max_per_peer {
            return Err(errcode::PEER_LIMIT);
        }
        let ttl_secs = if ttl_secs == 0 { DEFAULT_TTL_SECS } else { ttl_secs.min(MAX_TTL_SECS) };
        let cid = self.next_circuit;
        self.next_circuit += 1;
        self.circuit_load.insert(owner.to_string(), load + 1);
        self.circuits.insert(
            cid,
            CircuitSlot { owner: owner.to_string(), expires: Instant::now() + Duration::from_secs(ttl_secs), pending: None },
        );
        Ok(cid)
    }

    /// 配额检查 + 配对裁决，单临界区完成（Park 时流已收进槽内，无竞态窗口）。
    pub(crate) fn on_connect(&mut self, joiner: &str, cid: u64, max_per_peer: usize, stream: BoxedStream) -> CircuitOutcome {
        let Some(slot) = self.circuits.get_mut(&cid) else {
            return CircuitOutcome::Rejected(errcode::UNKNOWN_CIRCUIT, format!("circuit {cid} not found"), stream);
        };
        if Instant::now() >= slot.expires {
            let slot = self.circuits.remove(&cid).expect("just fetched");
            self.release_circuit_load(&slot.owner);
            if let Some(p) = slot.pending {
                self.release_circuit_load(&p.peer);
            }
            return CircuitOutcome::Rejected(errcode::CIRCUIT_EXPIRED, format!("circuit {cid} expired"), stream);
        }
        let load = self.circuit_load.get(joiner).copied().unwrap_or(0);
        if load >= max_per_peer {
            return CircuitOutcome::Rejected(errcode::PEER_LIMIT, "per-peer circuit quota exceeded".into(), stream);
        }
        self.circuit_load.insert(joiner.to_string(), load + 1);
        match slot.pending.take() {
            Some(p) => CircuitOutcome::Paired(p, stream),
            None => {
                slot.pending = Some(PendingStream { peer: joiner.to_string(), stream });
                CircuitOutcome::Parked
            }
        }
    }

    /// 桥接/清扫结束后回吐一个单位的电路配额。
    pub(crate) fn release_circuit_load(&mut self, peer: &str) {
        if let Some(n) = self.circuit_load.get_mut(peer) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                self.circuit_load.remove(peer);
            }
        }
    }

    /// 清扫到期电路，返回被丢弃者（含待配对流归属者，配额一并回吐）。
    pub(crate) fn sweep_expired(&mut self, now: Instant) -> Vec<ExpiredCircuit> {
        let expired: Vec<u64> = self
            .circuits
            .iter()
            .filter(|(_, s)| now >= s.expires)
            .map(|(cid, _)| *cid)
            .collect();
        expired
            .into_iter()
            .map(|cid| {
                let slot = self.circuits.remove(&cid).expect("checked above");
                self.release_circuit_load(&slot.owner);
                ExpiredCircuit {
                    cid,
                    holder: slot.pending.map(|p| {
                        self.release_circuit_load(&p.peer);
                        p.peer
                    }),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_stream() -> BoxedStream {
        let (a, _b) = tokio::io::duplex(16);
        Box::new(a)
    }

    #[test]
    fn link_quota_enforced() {
        let mut st = RelayState::new();
        assert!(st.register_link("p", 2));
        assert!(st.register_link("p", 2));
        assert!(!st.register_link("p", 2));
        st.unregister_link("p");
        assert!(st.register_link("p", 2));
    }

    #[test]
    fn circuit_park_then_pair() {
        let mut st = RelayState::new();
        let cid = st.issue_circuit("a", 60, 8).unwrap();
        assert!(matches!(st.on_connect("a", cid, 8, dummy_stream()), CircuitOutcome::Parked));
        assert!(matches!(st.on_connect("b", cid, 8, dummy_stream()), CircuitOutcome::Paired(_, _)));
    }

    #[test]
    fn unknown_and_limited_decisions() {
        let mut st = RelayState::new();
        assert!(matches!(
            st.on_connect("x", 999, 8, dummy_stream()),
            CircuitOutcome::Rejected(errcode::UNKNOWN_CIRCUIT, _, _)
        ));
        let cid = st.issue_circuit("a", 60, 1).unwrap();
        assert!(matches!(
            st.on_connect("a", cid, 1, dummy_stream()),
            CircuitOutcome::Rejected(errcode::PEER_LIMIT, _, _)
        ));
    }

    #[test]
    fn expired_circuit_swept_with_quota_release() {
        let mut st = RelayState::new();
        let cid = st.issue_circuit("a", 1, 8).unwrap();
        assert!(matches!(st.on_connect("b", cid, 8, dummy_stream()), CircuitOutcome::Parked));
        let dropped = st.sweep_expired(Instant::now() + Duration::from_secs(2));
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].holder.as_deref(), Some("b"));
        // owner/b 双方配额均已回吐
        assert!(st.issue_circuit("a", 60, 1).is_ok());
        assert!(st.issue_circuit("b", 60, 1).is_ok());
    }
}
