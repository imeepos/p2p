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
pub use swarm::filter_loopback;
pub use swarm::{AddrSource, Swarm, SwarmConfig, SwarmFactory};

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
    /// 降级链一跳的结果（M3）：ok=false 必须带原因，禁止静默降级（design §12）。
    DialHop {
        peer: PeerId,
        hop: DialHop,
        ok: bool,
        detail: String,
    },
}

/// 降级链一跳（design §7.3，M3 新增）：按序 直连 → 打洞 → 中继电路。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialHop {
    /// 直连（按地址簿逐一尝试）。
    Direct,
    /// 打洞（relay 信令换地址后双向探测）。
    Punch,
    /// 中继电路兜底。
    Relay,
}

/// 连接门禁：通信层 allow/deny（不是业务鉴权，design §6）。
#[async_trait::async_trait]
pub trait ConnectionGate: Send + Sync {
    async fn allow(&self, peer: &PeerId) -> bool;
}
