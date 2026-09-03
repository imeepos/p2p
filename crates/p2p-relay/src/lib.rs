//! 中继与打洞契约（design §7.3）：bootstrap 公网节点兼任 relay。
//!
//! relay 只桥接两条已认证连接之间的密文字节流，无法解密业务数据。
//! 服务端与客户端实现归中继会话 R；本文件冻结协议 ID 与接缝。
//! 防滥用红线：每 Peer 限速/限连接，超额断链并留观测信号。

/// 底座内置控制协议 ID（design §5.4），全底座统一，禁止重复定义。
pub mod proto_ids {
    pub const IDENTIFY: &str = "/p2p-base/identify/1";
    pub const PING: &str = "/p2p-base/ping/1";
    pub const RENDEZVOUS: &str = "/p2p-base/rendezvous/1";
    pub const RELAY: &str = "/p2p-base/relay/1";
    pub const CIRCUIT: &str = "/p2p-base/circuit/1";
}

/// 中继服务端：接受 reserve/connect，桥接两条连接的密文字节流。
#[async_trait::async_trait]
pub trait RelayService: Send + Sync {
    /// 阻塞运行直至致命错误（端口占用等）；运行期错误只记日志不断服务。
    async fn serve(self: std::sync::Arc<Self>) -> std::io::Result<()>;
}

/// 中继会话上的逻辑链路标识（reserve 成功后由服务端发放）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CircuitId(pub u64);

// ===== 以下为中继会话 R 的实现：冻结契约只增不改 =====

mod circuit;
pub mod client;
mod control;
pub mod error;
pub mod frame;
mod health;
mod keepalive;
mod lifecycle;
pub mod limits;
pub mod link;
mod load;
pub mod messages;
mod metrics;
pub mod punch;
pub mod service;
mod slots;
mod state;
pub mod testutil;

pub use client::{RelayClient, RelayEvent};
pub use error::RelayError;
pub use frame::{read_msg, write_msg, write_reject, MAX_FRAME};
pub use health::{RelayHealth, RelayHealthSnapshot};
pub use keepalive::RelayKeepalive;
pub use limits::{PeerBuckets, RateBucket, RateLimitedStream, RelayLimits};
pub use link::{LinkSource, RelayLink};
pub use messages::{
    errcode, relay_msg, Bound, Connect, PunchAck, PunchReq, Reject, RelayMsg, Reserve, Reserved,
};
pub use metrics::RelayMetricsSnapshot;
pub use punch::{PunchPhase, PunchSession};
pub use service::RelayServiceImpl;
