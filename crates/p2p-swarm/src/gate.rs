//! 门禁适配：把同步闭包适配为 [ConnectionGate]（design §4 `node.gate(...)` 便利形式）。

use p2p_identity::PeerId;

use crate::ConnectionGate;

/// 闭包门禁：`gate_fn(|peer| allowlist.contains(peer))`。
pub struct GateFn<F> {
    f: F,
}

impl<F> GateFn<F>
where
    F: Fn(&PeerId) -> bool + Send + Sync,
{
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

#[async_trait::async_trait]
impl<F> ConnectionGate for GateFn<F>
where
    F: Fn(&PeerId) -> bool + Send + Sync,
{
    async fn allow(&self, peer: &PeerId) -> bool {
        (self.f)(peer)
    }
}

/// 便捷构造：同步闭包 → [ConnectionGate]。
pub fn gate_fn<F>(f: F) -> GateFn<F>
where
    F: Fn(&PeerId) -> bool + Send + Sync,
{
    GateFn::new(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_identity::PeerId;

    #[tokio::test]
    async fn closure_decides_admission() {
        let allowed = PeerId::from_bytes([1; 32]);
        let gate = gate_fn(move |peer: &PeerId| *peer == allowed);
        assert!(gate.allow(&PeerId::from_bytes([1; 32])).await);
        assert!(!gate.allow(&PeerId::from_bytes([2; 32])).await);
    }
}
