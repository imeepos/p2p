//! 对外 facade：Node / NodeBuilder（design §4 的 API 表面）。
//!
//! build() 装配身份持久化（目录 0700）、QUIC/TCP 监听、mDNS/rendezvous 发现、
//! 地址观测反射、swarm 连接编排与 handler 注册表；业务只面对 [Node] API。

mod assembly;
mod discovery;
mod node;
mod observe;
mod rendezvous;
// W2b/CC4 起 pub：GUI 静态对端簿命令面复用 StaticPeersFile 持久化原语
//（load/upsert/remove/entries；0600、tmp+rename 语义不变，零协议/装配改动）。
pub mod static_peers;

#[cfg(test)]
mod assembly_tests;

use std::path::PathBuf;
use std::time::Duration;

pub use node::Node;
pub use p2p_identity::PeerId;
pub use p2p_mux::BoxedStream;
pub use p2p_protocol::{ProtocolHandler, ProtocolId};
pub use p2p_swarm::{gate_fn, ConnectionGate, GateFn, NodeEvent};
// 服务注册表 crate 面（service-registry-design）：无法直接依赖 p2p-service 的
// 宿主（apps/cli/Cargo.toml 归 B3，装配面 daemon.rs）经本 facade 复用同一真值源。
pub use p2p_service;
pub use rendezvous::TransportLink;

/// 服务总控显式化型开关（service-registry-design §2）：经 p2p-service
/// resolve 后的显式布尔，装配期「开关 AND 配置」双条件的开关位；默认全 on
/// = 现行为零变化。收编型 mdns/lan_only 沿用既有 enable_mdns/lan_only 字段。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceSwitches {
    /// net.rendezvous_register：false 时 bootstrap 非空也不接线 rendezvous 注册。
    pub rendezvous_register: bool,
    /// net.relay：false 时降级链剥离 relay，止于直连/打洞。
    pub relay: bool,
    /// net.observe：false 时跳过出站地址观测。
    pub observe: bool,
    /// serve.rendezvous_server：false 时不装配 RendezvousServer（public_only 语义保留）。
    pub rendezvous_server: bool,
}

impl Default for ServiceSwitches {
    fn default() -> Self {
        Self {
            rendezvous_register: true,
            relay: true,
            observe: true,
            rendezvous_server: true,
        }
    }
}

/// 节点配置（design §4 builder 入参）。
#[derive(Clone, Debug)]
pub struct NodeConfig {
    /// 0 = 随机端口。
    pub quic_port: u16,
    pub tcp_port: u16,
    /// rendezvous bootstrap 地址（ip/u端口 或 ip/t端口）；空则跳过接线并留日志。
    pub bootstrap: Vec<String>,
    pub enable_mdns: bool,
    /// 预留：单帧上限；协议层当前为固定 1 MiB 常量，超限即帧错误。
    pub max_frame_size: u32,
    /// 身份数据目录，默认 ./p2p-data（目录权限 0700）。
    pub data_dir: PathBuf,
    /// relay 服务地址（design §7.3 降级链 2/3 跳）；空则降级链止于直连。
    pub relay_addrs: Vec<String>,
    /// 对外宣告地址（打洞信令，design §7.2 观测地址）；空则用监听地址。
    pub advertised_addrs: Vec<String>,
    /// 观测反射口（design §7.2，bootstrap 角色节点启用）；None = 不启用。
    pub observation_port: Option<u16>,
    /// 观测口地址（ip:port），启动时学习自身公网映射地址；空则跳过观测。
    pub observation_addrs: Vec<String>,
    /// 观测探测有界重试：总尝试次数（含首次，BASE1 注册退化重试）。
    /// 单次 UDP 随机失败不得让注册地址集退化；耗尽显式告警并置降级状态。
    pub observe_attempts: u32,
    /// 观测探测首次失败后的退避间隔，之后逐次翻倍封顶 16x（次数/间隔可配）。
    pub observe_interval: Duration,
    /// rendezvous 服务端公共策略：拒收全不可路由注册；公共 bootstrap 部署开启。
    pub rendezvous_public_only: bool,
    /// 静态对端登记文件（社交化发现 P1）：启动载入 + upsert 落盘；None = 不启用。
    pub static_peers_file: Option<PathBuf>,
    /// 仅局域网模式（F8）：true 时装配期剥离全部公网端点（bootstrap/relay/
    /// observation），仅保留局域网发现与直连；默认 false 保持现行为。
    pub lan_only: bool,
    /// 服务总控显式化型开关（service-registry-design §2，B1）：默认全 on。
    pub service_switches: ServiceSwitches,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            quic_port: 0,
            tcp_port: 0,
            bootstrap: Vec::new(),
            enable_mdns: true,
            max_frame_size: 1 << 20,
            data_dir: PathBuf::from("./p2p-data"),
            relay_addrs: Vec::new(),
            advertised_addrs: Vec::new(),
            observation_port: None,
            observation_addrs: Vec::new(),
            observe_attempts: 3,
            observe_interval: Duration::from_millis(500),
            rendezvous_public_only: false,
            static_peers_file: None,
            lan_only: false,
            service_switches: ServiceSwitches::default(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol: {0}")]
    Protocol(#[from] p2p_protocol::ProtocolError),
    #[error("assembly: {0}")]
    Assembly(String),
}

pub struct NodeBuilder(NodeConfig);

impl NodeBuilder {
    pub fn new() -> Self {
        Self(NodeConfig::default())
    }

    pub fn quic_port(mut self, port: u16) -> Self {
        self.0.quic_port = port;
        self
    }

    pub fn tcp_port(mut self, port: u16) -> Self {
        self.0.tcp_port = port;
        self
    }

    pub fn bootstrap(mut self, addrs: Vec<String>) -> Self {
        self.0.bootstrap = addrs;
        self
    }

    pub fn mdns(mut self, on: bool) -> Self {
        self.0.enable_mdns = on;
        self
    }

    /// 身份数据目录（目录权限 0700，种子文件 0600）。
    pub fn data_dir(mut self, dir: PathBuf) -> Self {
        self.0.data_dir = dir;
        self
    }

    /// relay 服务地址（M3 降级链 2/3 跳入口）。
    pub fn relay_addrs(mut self, addrs: Vec<String>) -> Self {
        self.0.relay_addrs = addrs;
        self
    }

    /// 对外宣告地址（打洞信令携带；NAT 场景填观测地址）。
    pub fn advertised_addrs(mut self, addrs: Vec<String>) -> Self {
        self.0.advertised_addrs = addrs;
        self
    }

    /// 启用观测反射器（bootstrap 角色节点；UDP 端口）。
    pub fn observation_responder(mut self, port: u16) -> Self {
        self.0.observation_port = Some(port);
        self
    }

    /// 观测口地址（ip:port），启动时学习自身公网映射地址并注册进 rendezvous。
    pub fn observation_addrs(mut self, addrs: Vec<String>) -> Self {
        self.0.observation_addrs = addrs;
        self
    }

    /// 观测探测总尝试次数（含首次，至少 1；BASE1 注册退化重试）。
    pub fn observe_attempts(mut self, attempts: u32) -> Self {
        self.0.observe_attempts = attempts;
        self
    }

    /// 观测探测首次失败后的退避间隔（之后逐次翻倍封顶 16x）。
    pub fn observe_interval(mut self, interval: Duration) -> Self {
        self.0.observe_interval = interval;
        self
    }

    /// rendezvous 服务端公共策略（E5）：公共 bootstrap 部署开启，拒收全
    /// loopback/link-local 注册；同机/单测部署保持默认宽松。
    pub fn rendezvous_public_only(mut self, public_only: bool) -> Self {
        self.0.rendezvous_public_only = public_only;
        self
    }

    /// 静态对端登记文件（社交化发现 P1）：启动载入 AddressBook（Manual
    /// 来源），配合 [`Node::upsert_static_peer`] 落盘，重启后可直拨。
    pub fn static_peers_file(mut self, path: PathBuf) -> Self {
        self.0.static_peers_file = Some(path);
        self
    }

    /// 仅局域网模式（F8）：公网外联显式关闭，详情见 [NodeConfig::lan_only]。
    pub fn lan_only(mut self, on: bool) -> Self {
        self.0.lan_only = on;
        self
    }

    /// 服务总控显式化型开关（service-registry-design §2）：入参为 p2p-service
    /// resolve 后的显式布尔，默认全 on；生效时机=下次节点启动（不热更）。
    pub fn service_switches(mut self, switches: ServiceSwitches) -> Self {
        self.0.service_switches = switches;
        self
    }

    pub async fn build(self) -> Result<Node, NodeError> {
        assembly::build(self.0).await
    }
}

impl Default for NodeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
