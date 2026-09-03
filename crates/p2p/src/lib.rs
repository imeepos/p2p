//! 对外 facade：Node / NodeBuilder（design §4 的 API 表面）。
//!
//! build() 装配身份持久化（目录 0700）、QUIC/TCP 监听、mDNS/rendezvous 发现、
//! 地址观测反射、swarm 连接编排与 handler 注册表；业务只面对 [Node] API。

mod assembly;
mod discovery;
mod node;
mod observe;
mod rendezvous;

use std::path::PathBuf;

pub use node::Node;
pub use p2p_identity::PeerId;
pub use p2p_mux::BoxedStream;
pub use p2p_protocol::{ProtocolHandler, ProtocolId};
pub use p2p_swarm::{gate_fn, ConnectionGate, GateFn, NodeEvent};

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
    /// rendezvous 服务端公共策略：拒收全不可路由注册；公共 bootstrap 部署开启。
    pub rendezvous_public_only: bool,
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
            rendezvous_public_only: false,
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

    /// rendezvous 服务端公共策略（E5）：公共 bootstrap 部署开启，拒收全
    /// loopback/link-local 注册；同机/单测部署保持默认宽松。
    pub fn rendezvous_public_only(mut self, public_only: bool) -> Self {
        self.0.rendezvous_public_only = public_only;
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
