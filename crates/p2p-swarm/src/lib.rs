//! 连接编排契约（design §8/§9）：连接池、门禁、事件总线。
//!
//! 实现排在内核/协议会话合并之后（S 阶段），本文件冻结事件与门禁接缝。

use p2p_identity::PeerId;

/// 底座事件：业务只读。所有失败路径必须可见（禁止静默吞错，design §12）。
#[derive(Clone, Debug)]
pub enum NodeEvent {
    PeerDiscovered { peer: PeerId, addrs: Vec<String> },
    PeerConnected { peer: PeerId },
    PeerDisconnected { peer: PeerId },
    ListenFailed { addr: String, reason: String },
    DialFailed { peer: Option<PeerId>, reason: String },
    ProtocolViolation { peer: PeerId, reason: String },
}

/// 连接门禁：通信层 allow/deny（不是业务鉴权，design §6）。
#[async_trait::async_trait]
pub trait ConnectionGate: Send + Sync {
    async fn allow(&self, peer: &PeerId) -> bool;
}
