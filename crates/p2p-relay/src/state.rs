//! 服务端记账（纯内存，无 IO）：链路配额与控制通道注册。
//! 电路槽位与配对的记账在 slots 模块，两者共享 [RelayState] 的字段。

use std::collections::HashMap;
use std::sync::Arc;

use p2p_mux::BoxedStream;
use tokio::io::WriteHalf;
use tokio::sync::Mutex as AsyncMutex;

/// 控制流写半（读半由控制循环独占）。
pub(crate) type CtrlWrite = AsyncMutex<WriteHalf<BoxedStream>>;

/// 链路登记拒绝原因（审查 M5：全站总量上限抗 Sybil 稀释）。
#[derive(Debug)]
pub(crate) enum LinkReject {
    PeerQuota,
    GlobalQuota,
}

pub(crate) struct RelayState {
    pub(crate) links: HashMap<String, usize>,
    pub(crate) circuit_load: HashMap<String, usize>,
    pub(crate) circuits: HashMap<u64, crate::slots::CircuitSlot>,
    pub(crate) controls: HashMap<String, Arc<CtrlWrite>>,
}

impl RelayState {
    pub(crate) fn new() -> Self {
        Self {
            links: HashMap::new(),
            circuit_load: HashMap::new(),
            circuits: HashMap::new(),
            controls: HashMap::new(),
        }
    }

    /// 登记一条入站链路；超每 Peer 配额或全站总量均拒绝。
    pub(crate) fn register_link(
        &mut self,
        peer: &str,
        max_per_peer: usize,
        max_total: usize,
    ) -> Result<(), LinkReject> {
        if self.links.values().sum::<usize>() >= max_total {
            return Err(LinkReject::GlobalQuota);
        }
        let count = self.links.get(peer).copied().unwrap_or(0);
        if count >= max_per_peer {
            return Err(LinkReject::PeerQuota);
        }
        self.links.insert(peer.to_string(), count + 1);
        Ok(())
    }

    pub(crate) fn unregister_link(&mut self, peer: &str) {
        if let Some(n) = self.links.get_mut(peer) {
            *n -= 1;
            if *n == 0 {
                self.links.remove(peer);
            }
        }
    }

    /// 既无链路也无在途电路流即为闲置（带宽桶随之回收，防表只增不减）。
    pub(crate) fn peer_idle(&self, peer: &str) -> bool {
        !self.links.contains_key(peer) && !self.circuit_load.contains_key(peer)
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
        if self
            .controls
            .get(peer)
            .is_some_and(|h| Arc::ptr_eq(h, half))
        {
            self.controls.remove(peer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_quota_enforced() {
        let mut st = RelayState::new();
        assert!(st.register_link("p", 2, 8).is_ok());
        assert!(st.register_link("p", 2, 8).is_ok());
        assert!(matches!(
            st.register_link("p", 2, 8),
            Err(LinkReject::PeerQuota)
        ));
        st.unregister_link("p");
        assert!(st.register_link("p", 2, 8).is_ok());
    }

    #[test]
    fn global_link_cap_enforced() {
        let mut st = RelayState::new();
        assert!(st.register_link("a", 8, 1).is_ok());
        assert!(matches!(
            st.register_link("b", 8, 1),
            Err(LinkReject::GlobalQuota)
        ));
        st.unregister_link("a");
        assert!(st.register_link("b", 8, 1).is_ok());
    }

    #[test]
    fn peer_idle_tracks_links_and_circuit_load() {
        let mut st = RelayState::new();
        st.circuit_load.insert("p".into(), 1);
        assert!(!st.peer_idle("p"));
        st.circuit_load.remove("p");
        assert!(st.peer_idle("p"));
    }
}
