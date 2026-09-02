//! 对外 facade：Node / NodeBuilder（design §4 的 API 表面）。
//!
//! 装配排在 S 阶段；当前 build 返回 [NodeError::NotYetAssembled]，
//! 本文件的作用是冻结对外 API 形状。

#[derive(Clone, Debug)]
pub struct NodeConfig {
    /// 0 = 随机端口。
    pub quic_port: u16,
    pub tcp_port: u16,
    pub bootstrap: Vec<String>,
    pub enable_mdns: bool,
    pub max_frame_size: u32,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            quic_port: 0,
            tcp_port: 0,
            bootstrap: Vec::new(),
            enable_mdns: true,
            max_frame_size: 1 << 20,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("facade 尚未装配（S 阶段实现）")]
    NotYetAssembled,
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

    pub async fn build(self) -> Result<Node, NodeError> {
        let _ = self.0;
        Err(NodeError::NotYetAssembled)
    }
}

impl Default for NodeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Node {
    /// S 阶段填充：swarm 句柄、handler 注册表、事件流。
    _priv: (),
}
