//! 对外 facade：Node / NodeBuilder（design §4 的 API 表面）。
//!
//! build() 装配身份持久化（目录 0700）、QUIC/TCP 监听、mDNS/rendezvous 发现、
//! swarm 连接编排与 handler 注册表；业务只面对 [Node] API。

mod assembly;
mod discovery;
mod node;
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
    /// rendezvous bootstrap 地址（`ip/u端口` 或 `ip/t端口`）；空则跳过接线并留日志。
    pub bootstrap: Vec<String>,
    pub enable_mdns: bool,
    /// 预留：单帧上限；协议层当前为固定 1 MiB 常量，超限即帧错误。
    pub max_frame_size: u32,
    /// 身份数据目录，默认 ./p2p-data（目录权限 0700）。
    pub data_dir: PathBuf,
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

    pub async fn build(self) -> Result<Node, NodeError> {
        assembly::build(self.0).await
    }
}

impl Default for NodeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
