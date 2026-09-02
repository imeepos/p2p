//! Swarm 装配参数与地址工具。

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use p2p_identity::Keypair;
use p2p_protocol::HandlerRegistry;
use p2p_transport::TransportAddr;

/// broadcast 事件通道容量；慢消费者落后超过即 Lagged，由其自行处理。
pub(crate) const EVENT_CAPACITY: usize = 128;

/// Swarm 装配参数。
pub struct SwarmConfig {
    pub keypair: Arc<Keypair>,
    /// 0 = 随机端口。
    pub quic_port: u16,
    pub tcp_port: u16,
    pub registry: Arc<HandlerRegistry>,
    /// relay 服务地址（`ip/u端口` 或 `ip/t端口`）；空则降级链只到直连为止。
    pub relay_addrs: Vec<TransportAddr>,
    /// 对外宣告地址（打洞信令携带，design §7.2 观测地址）；空则用监听地址。
    pub advertised_addrs: Vec<TransportAddr>,
}

/// 未指定 IP（0.0.0.0/::）替换为 127.0.0.1，保证地址簿里的监听地址可直连。
pub(crate) fn to_transport(addr: SocketAddr, quic: bool) -> TransportAddr {
    let ip = if addr.ip().is_unspecified() {
        IpAddr::from([127, 0, 0, 1])
    } else {
        addr.ip()
    };
    if quic {
        TransportAddr::Quic {
            ip,
            port: addr.port(),
        }
    } else {
        TransportAddr::Tcp {
            ip,
            port: addr.port(),
        }
    }
}
