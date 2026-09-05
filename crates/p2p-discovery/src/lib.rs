//! 发现层契约（design §7）：mDNS(局域网) 与 rendezvous(跨网) 的统一接缝。
//!
//! 实现归发现会话 D。红线：rendezvous 注册必须用身份私钥签名（防 PeerId 劫持）；
//! bootstrap 不可达时降级走 last-known-good 缓存；失败路径必须发 [DiscoveryEvent::Failed]。

use std::sync::Arc;
use std::time::Instant;

use p2p_identity::PeerId;
use p2p_transport::TransportAddr;
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
pub struct DiscoveredPeer {
    pub peer: PeerId,
    pub addrs: Vec<TransportAddr>,
    pub source: Source,
    /// None 表示无 TTL（如静态配置）。
    pub expires_at: Option<Instant>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Mdns,
    Rendezvous,
    Cache,
}

#[derive(Clone, Debug)]
pub enum DiscoveryEvent {
    Discovered(DiscoveredPeer),
    Expired(PeerId),
    Failed { source: Source, reason: String },
}

/// 发现源：以独立任务运行，事件推入 channel；实现不得 panic，失败走 Failed 事件。
#[async_trait::async_trait]
pub trait Discovery: Send + Sync {
    fn name(&self) -> &'static str;
    async fn run(self: Arc<Self>, events: mpsc::Sender<DiscoveryEvent>);
}

/// 带 TTL 的地址缓存：last-known-good 降级的基础设施（D 会话实现）。
#[async_trait::async_trait]
pub trait AddrCache: Send + Sync {
    fn put(&self, peer: PeerId, addrs: Vec<TransportAddr>, ttl: std::time::Duration);
    /// 只返回未过期条目；过期即清除并发 Expired 语义由调用方处理。
    fn get(&self, peer: &PeerId) -> Option<Vec<TransportAddr>>;
    fn evict_expired(&self) -> Vec<PeerId>;
}

// ---- D 会话实现（追加，冻结契约未改）----
pub mod cache;
pub mod mdns;
pub mod rendezvous;
pub mod retry;

pub use cache::MemCache;
pub use mdns::{MdnsConfig, MdnsDiscovery};
pub use rendezvous::{
    RendezvousClient, RendezvousConfig, RendezvousError, RendezvousLink, RendezvousRegistry,
};
pub use retry::{retry_bounded, RetryExhausted, RetryPolicy};
