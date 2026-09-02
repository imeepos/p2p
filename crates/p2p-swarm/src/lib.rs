//! 连接编排（design §8/§9/§12）：连接池、拨号器、门禁、事件总线、退避工具。
//!
//! Swarm 绑定 QUIC/TCP 监听并驱动 accept 循环；出站按地址簿顺序直连；
//! 所有失败路径发 [NodeEvent] 或留日志，禁止静默（design §12）。

use p2p_identity::PeerId;

mod backoff;
mod gate;
mod pool;
mod swarm;

pub use backoff::Backoff;
pub use gate::{gate_fn, GateFn};
pub use pool::ConnectionPool;
pub use swarm::{Swarm, SwarmConfig, SwarmFactory};

/// 底座事件：业务只读。所有失败路径必须可见（禁止静默吞错，design §12）。
#[derive(Clone, Debug)]
pub enum NodeEvent {
    PeerDiscovered {
        peer: PeerId,
        addrs: Vec<String>,
    },
    PeerConnected {
        peer: PeerId,
    },
    PeerDisconnected {
        peer: PeerId,
    },
    ListenFailed {
        addr: String,
        reason: String,
    },
    DialFailed {
        peer: Option<PeerId>,
        reason: String,
    },
    ProtocolViolation {
        peer: PeerId,
        reason: String,
    },
}

/// 连接门禁：通信层 allow/deny（不是业务鉴权，design §6）。
#[async_trait::async_trait]
pub trait ConnectionGate: Send + Sync {
    async fn allow(&self, peer: &PeerId) -> bool;
}