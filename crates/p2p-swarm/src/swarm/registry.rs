//! handler 注册表与门禁挂载（自 mod.rs 搬出，E6）：复制-改-换的注册路径
//! 与门禁裁决，逻辑不变；为 swarm/mod.rs 腾出 300 行余量。

use std::sync::Arc;

use p2p_identity::PeerId;
use p2p_protocol::{HandlerRegistry, ProtocolHandler};

use super::Swarm;

impl Swarm {
    /// 注册协议 handler：复制-改-换，进行中的分发继续使用旧快照。
    pub fn register(&self, handler: Arc<dyn ProtocolHandler>) {
        let mut guard = self.registry.lock().expect("registry lock");
        let mut next = HandlerRegistry::default();
        for id in guard.protocols() {
            if let Some(h) = guard.get(&id) {
                next.register(h);
            }
        }
        next.register(handler);
        *guard = Arc::new(next);
    }

    pub fn set_gate(&self, gate: Arc<dyn crate::ConnectionGate>) {
        *self.gate.lock().expect("gate lock") = Some(gate);
    }

    /// 门禁裁决：未配置即放行；锁外 await，不阻塞注册路径。
    pub(super) async fn gate_allows(&self, peer: PeerId) -> bool {
        let gate = self.gate.lock().expect("gate lock").clone();
        match gate {
            Some(g) => g.allow(&peer).await,
            None => true,
        }
    }
}
