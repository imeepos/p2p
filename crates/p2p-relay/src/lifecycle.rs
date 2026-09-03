//! 注册载体电路生命周期：信令面消失即回收未桥接的电路，配额不随 churn 泄漏。
//!
//! 缺陷背景（E4）：控制注册本身占用一次 reserve（TTL 最长 3600s），对端消失后
//! 槽位不回收，churn 下按 per-peer 配额（32）滚动自锁。两条回收触发器：
//! 控制流关闭（流级 EOF/协议断）与链路归零（对端整体消失，覆盖读半被
//! 悬挂任务钉住、EOF 传播不到的路径）。语义对齐 swarm 侧约定「控制流存活
//! 期内注册持续有效」；已桥接电路承载在途数据，不受信令面断开影响。

use crate::service::RelayServiceImpl;
use crate::slots::{CircuitPhase, PendingStream};
use crate::state::RelayState;

/// 控制流关闭时被回收的电路；holder 是待配对流（需要显式拒绝信号）。
pub(crate) struct ReleasedCircuit {
    pub cid: u64,
    pub holder: Option<PendingStream>,
}

impl RelayState {
    /// 回收指定控制流代次下所有未桥接电路，回吐 owner 与待配对方配额。
    /// 桥接中的电路不动：数据面存活不依赖信令面。
    pub(crate) fn release_control_circuits(&mut self, epoch: u64) -> Vec<ReleasedCircuit> {
        let stale: Vec<u64> = self
            .circuits
            .iter()
            .filter(|(_, s)| s.ctrl_epoch == epoch && s.phase != CircuitPhase::Bridged)
            .map(|(cid, _)| *cid)
            .collect();
        stale
            .into_iter()
            .map(|cid| {
                let slot = self.circuits.remove(&cid).expect("checked above");
                self.release_circuit_load(&slot.owner);
                let holder = slot.pending.inspect(|p| self.release_circuit_load(&p.peer));
                ReleasedCircuit { cid, holder }
            })
            .collect()
    }

    /// 回收指定 Peer 全部未桥接电路（链路归零＝对端消失的兜底回收）。
    pub(crate) fn release_peer_circuits(&mut self, peer: &str) -> Vec<ReleasedCircuit> {
        let stale: Vec<u64> = self
            .circuits
            .iter()
            .filter(|(_, s)| s.owner == peer && s.phase != CircuitPhase::Bridged)
            .map(|(cid, _)| *cid)
            .collect();
        stale
            .into_iter()
            .map(|cid| {
                let slot = self.circuits.remove(&cid).expect("checked above");
                self.release_circuit_load(&slot.owner);
                let holder = slot.pending.inspect(|p| self.release_circuit_load(&p.peer));
                ReleasedCircuit { cid, holder }
            })
            .collect()
    }
}

impl RelayServiceImpl {
    /// 控制流关闭：回收该流发放的未桥接电路（流级触发器）。
    pub(crate) async fn release_epoch_circuits(&self, peer: &str, epoch: u64) {
        let released = self.lock_state().release_control_circuits(epoch);
        self.discard(peer, released, "control stream closed").await;
    }

    /// 链路归零：回收该 Peer 全部未桥接电路（对端消失触发器）。
    pub(crate) async fn release_all_circuits_of_peer(&self, peer: &str) {
        let released = self.lock_state().release_peer_circuits(peer);
        self.discard(peer, released, "peer links gone").await;
    }

    /// 统一落地：待配对流给显式拒绝再关流，裸槽位只留观测日志。
    async fn discard(&self, peer: &str, released: Vec<ReleasedCircuit>, cause: &str) {
        if !released.is_empty() {
            self.lock_state()
                .metrics
                .count_recycled(released.len() as u64);
        }
        for r in released {
            match r.holder {
                Some(mut pending) => {
                    let _ = crate::frame::write_reject(
                        &mut pending.stream,
                        crate::messages::errcode::CIRCUIT_EXPIRED,
                        "signaling gone; reservation released",
                    )
                    .await;
                    tracing::warn!(peer = %peer, circuit = r.cid, holder = %pending.peer, cause, "circuit released; parked holder rejected");
                }
                None => {
                    tracing::debug!(peer = %peer, circuit = r.cid, cause, "registration circuit released");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slots::CircuitOutcome;

    fn dummy_stream() -> p2p_mux::BoxedStream {
        let (a, _b) = tokio::io::duplex(16);
        Box::new(a)
    }

    #[test]
    fn bare_registration_circuit_released_with_quota() {
        let mut st = RelayState::new();
        let cid = st.issue_circuit("a", "", 3600, 1, 8, 7).unwrap();
        let released = st.release_control_circuits(7);
        assert_eq!(released.len(), 1);
        assert_eq!(released[0].cid, cid);
        assert!(released[0].holder.is_none(), "bare slot has no holder");
        // 配额已回吐：同配额 1 可再次发放
        assert!(st.issue_circuit("a", "", 3600, 1, 8, 7).is_ok());
    }

    #[test]
    fn parked_holder_rejected_and_quota_released() {
        let mut st = RelayState::new();
        let cid = st.issue_circuit("a", "b", 60, 8, 8, 3).unwrap();
        assert!(matches!(
            st.on_connect("b", cid, 8, dummy_stream()),
            CircuitOutcome::Parked
        ));
        let released = st.release_control_circuits(3);
        assert_eq!(released.len(), 1);
        let holder = released[0].holder.as_ref().expect("parked holder returned");
        assert_eq!(holder.peer, "b");
        // 双方配额均回吐
        assert!(st.issue_circuit("a", "", 60, 1, 8, 3).is_ok());
        assert!(st.issue_circuit("b", "", 60, 1, 8, 3).is_ok());
    }

    #[test]
    fn bridged_circuit_survives_control_close() {
        let mut st = RelayState::new();
        let cid = st.issue_circuit("a", "b", 60, 8, 8, 5).unwrap();
        assert!(matches!(
            st.on_connect("a", cid, 8, dummy_stream()),
            CircuitOutcome::Parked
        ));
        assert!(matches!(
            st.on_connect("b", cid, 8, dummy_stream()),
            CircuitOutcome::Paired(_, _)
        ));
        assert!(
            st.release_control_circuits(5).is_empty(),
            "bridged circuit must not die with its control stream"
        );
        assert!(
            st.circuits.contains_key(&cid),
            "slot remains until TTL sweep"
        );
    }

    #[test]
    fn other_epoch_circuits_untouched() {
        let mut st = RelayState::new();
        assert!(st.issue_circuit("a", "", 60, 8, 8, 1).is_ok());
        assert!(st.release_control_circuits(2).is_empty());
        assert_eq!(st.circuits.len(), 1, "其他代次电路不受影响");
    }
}
