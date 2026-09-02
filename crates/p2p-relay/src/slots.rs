//! 电路槽位记账：发放（CSPRNG 电路号 + 接入白名单）、配对裁决、到期清扫。

use std::time::{Duration, Instant};

use p2p_mux::BoxedStream;

use crate::messages::errcode;
use crate::state::RelayState;

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
    /// 允许接入的 PeerId；空串 = 仅 owner 可接入（审查 M2：接入须校验）。
    pub allowed_joiner: String,
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

impl RelayState {
    /// 发放电路：CSPRNG 生成不可枚举 cid（审查 M2）；超配额返回错误码。
    /// allowed_joiner 为空 = 仅 owner 可接入。TTL 0 用缺省值，超上限截断。
    pub(crate) fn issue_circuit(
        &mut self,
        owner: &str,
        allowed_joiner: &str,
        ttl_secs: u64,
        max_per_peer: usize,
    ) -> Result<u64, u32> {
        let load = self.circuit_load.get(owner).copied().unwrap_or(0);
        if load >= max_per_peer {
            return Err(errcode::PEER_LIMIT);
        }
        let ttl_secs = if ttl_secs == 0 {
            DEFAULT_TTL_SECS
        } else {
            ttl_secs.min(MAX_TTL_SECS)
        };
        let cid = loop {
            // ThreadRng 为 ChaCha12 CSPRNG；u64 撞号概率可忽略，重取即可
            let cid = rand::random::<u64>();
            if cid != 0 && !self.circuits.contains_key(&cid) {
                break cid;
            }
        };
        self.circuit_load.insert(owner.to_string(), load + 1);
        self.circuits.insert(
            cid,
            CircuitSlot {
                owner: owner.to_string(),
                allowed_joiner: allowed_joiner.to_string(),
                expires: Instant::now() + Duration::from_secs(ttl_secs),
                pending: None,
            },
        );
        Ok(cid)
    }

    /// 属主校验 + 配额检查 + 配对裁决，单临界区完成（Park 时流已收进槽内）。
    pub(crate) fn on_connect(
        &mut self,
        joiner: &str,
        cid: u64,
        max_per_peer: usize,
        stream: BoxedStream,
    ) -> CircuitOutcome {
        let Some(slot) = self.circuits.get_mut(&cid) else {
            return CircuitOutcome::Rejected(
                errcode::UNKNOWN_CIRCUIT,
                format!("circuit {cid} not found"),
                stream,
            );
        };
        if Instant::now() >= slot.expires {
            let slot = self.circuits.remove(&cid).expect("just fetched");
            self.release_circuit_load(&slot.owner);
            if let Some(p) = slot.pending {
                self.release_circuit_load(&p.peer);
            }
            return CircuitOutcome::Rejected(
                errcode::CIRCUIT_EXPIRED,
                format!("circuit {cid} expired"),
                stream,
            );
        }
        let authorized = joiner == slot.owner
            || (!slot.allowed_joiner.is_empty() && joiner == slot.allowed_joiner);
        if !authorized {
            return CircuitOutcome::Rejected(
                errcode::FORBIDDEN_JOINER,
                format!("peer {joiner} not allowed on circuit {cid}"),
                stream,
            );
        }
        let load = self.circuit_load.get(joiner).copied().unwrap_or(0);
        if load >= max_per_peer {
            return CircuitOutcome::Rejected(
                errcode::PEER_LIMIT,
                "per-peer circuit quota exceeded".into(),
                stream,
            );
        }
        self.circuit_load.insert(joiner.to_string(), load + 1);
        match slot.pending.take() {
            Some(p) => CircuitOutcome::Paired(p, stream),
            None => {
                slot.pending = Some(PendingStream {
                    peer: joiner.to_string(),
                    stream,
                });
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
    fn cid_unpredictable_and_unique() {
        let mut st = RelayState::new();
        let a = st.issue_circuit("a", "", 60, 8).unwrap();
        let b = st.issue_circuit("a", "", 60, 8).unwrap();
        assert_ne!(a, b);
        assert_ne!(a, 1, "cid 不再顺序自增");
    }

    #[test]
    fn foreign_joiner_rejected_owner_and_declared_allowed() {
        let mut st = RelayState::new();
        let cid = st.issue_circuit("a", "peer-b", 60, 8).unwrap();
        assert!(matches!(
            st.on_connect("peer-e", cid, 8, dummy_stream()),
            CircuitOutcome::Rejected(errcode::FORBIDDEN_JOINER, _, _)
        ));
        assert!(matches!(
            st.on_connect("a", cid, 8, dummy_stream()),
            CircuitOutcome::Parked
        ));
        assert!(matches!(
            st.on_connect("peer-b", cid, 8, dummy_stream()),
            CircuitOutcome::Paired(_, _)
        ));
    }

    #[test]
    fn owner_only_when_joiner_empty() {
        let mut st = RelayState::new();
        let cid = st.issue_circuit("a", "", 60, 8).unwrap();
        assert!(matches!(
            st.on_connect("b", cid, 8, dummy_stream()),
            CircuitOutcome::Rejected(errcode::FORBIDDEN_JOINER, _, _)
        ));
    }

    #[test]
    fn circuit_park_then_pair() {
        let mut st = RelayState::new();
        let cid = st.issue_circuit("a", "b", 60, 8).unwrap();
        assert!(matches!(
            st.on_connect("a", cid, 8, dummy_stream()),
            CircuitOutcome::Parked
        ));
        assert!(matches!(
            st.on_connect("b", cid, 8, dummy_stream()),
            CircuitOutcome::Paired(_, _)
        ));
    }

    #[test]
    fn unknown_and_limited_decisions() {
        let mut st = RelayState::new();
        assert!(matches!(
            st.on_connect("x", 999, 8, dummy_stream()),
            CircuitOutcome::Rejected(errcode::UNKNOWN_CIRCUIT, _, _)
        ));
        let cid = st.issue_circuit("a", "", 60, 1).unwrap();
        assert!(matches!(
            st.on_connect("a", cid, 1, dummy_stream()),
            CircuitOutcome::Rejected(errcode::PEER_LIMIT, _, _)
        ));
    }

    #[test]
    fn expired_circuit_swept_with_quota_release() {
        let mut st = RelayState::new();
        let cid = st.issue_circuit("a", "b", 1, 8).unwrap();
        assert!(matches!(
            st.on_connect("b", cid, 8, dummy_stream()),
            CircuitOutcome::Parked
        ));
        let dropped = st.sweep_expired(Instant::now() + Duration::from_secs(2));
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].holder.as_deref(), Some("b"));
        // owner/b 双方配额均已回吐
        assert!(st.issue_circuit("a", "", 60, 1).is_ok());
        assert!(st.issue_circuit("b", "", 60, 1).is_ok());
    }
}
