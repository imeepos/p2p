//! [Node]：design §4 的业务 API 表面。

use std::sync::Arc;
use std::time::Duration;

use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{
    open_with_protocol, ProtocolHandler, ProtocolId, RequestResponse, RequestResponseClient,
    StreamFactory,
};
use p2p_swarm::{ConnectionGate, NodeEvent, Swarm, SwarmFactory};
use tokio::sync::broadcast;

use crate::rendezvous::parse_transport_addr;
use crate::{NodeBuilder, NodeError};

pub struct Node {
    swarm: Arc<Swarm>,
}

impl Node {
    pub(crate) fn new(swarm: Arc<Swarm>) -> Self {
        Self { swarm }
    }

    /// 构建入口（design §4：Node::builder()）。
    pub fn builder() -> NodeBuilder {
        NodeBuilder::new()
    }

    pub fn local_peer_id(&self) -> PeerId {
        self.swarm.local_peer_id()
    }

    /// 事件订阅：发现/连接/断开/拨号失败/协议违规（design §4/§12）。
    pub fn events(&self) -> broadcast::Receiver<NodeEvent> {
        self.swarm.subscribe()
    }

    /// 注册业务协议 handler（底座只做路由，design §9）。
    pub fn handle_protocol(&self, handler: Arc<dyn ProtocolHandler>) {
        self.swarm.register(handler);
    }

    /// 连接门禁钩子（通信层安全，design §6）。
    pub fn set_gate(&self, gate: Arc<dyn ConnectionGate>) {
        self.swarm.set_gate(gate);
    }

    /// 幂等连接：池内复用或按地址簿直连。
    pub async fn connect(&self, peer: PeerId) -> Result<(), NodeError> {
        self.swarm.connect(peer).await?;
        Ok(())
    }

    /// 主动开流：首帧协议 ID 已写，之后即业务帧（design §5.1）。
    pub async fn new_stream(
        &self,
        peer: PeerId,
        protocol: ProtocolId,
    ) -> Result<BoxedStream, NodeError> {
        let raw = SwarmFactory(self.swarm.clone())
            .open_stream(&peer, &protocol)
            .await?;
        Ok(open_with_protocol(raw, &protocol).await?)
    }

    /// request-response 便捷原语（design §4/§11.3），一个 timeout 覆盖全程。
    pub async fn request(
        &self,
        peer: PeerId,
        protocol: ProtocolId,
        payload: Vec<u8>,
        timeout: Duration,
    ) -> Result<Vec<u8>, NodeError> {
        let client = RequestResponseClient::new(SwarmFactory(self.swarm.clone()));
        Ok(client.request(peer, protocol, payload, timeout).await?)
    }

    /// 静态登记对端地址（显式直连入口），新地址触发 PeerDiscovered 事件。
    pub fn add_peer_address(&self, peer: PeerId, addr: &str) -> Result<(), NodeError> {
        let addr = parse_transport_addr(addr)?;
        self.swarm.add_peer_addresses(peer, vec![addr]);
        Ok(())
    }

    /// 本节点监听地址（0.0.0.0 已换成 127.0.0.1，可直接用于拨号/登记）。
    pub fn listen_addrs(&self) -> Vec<String> {
        self.swarm.listen_addr_strings()
    }

    /// 关停：停 accept 循环并断开全部连接；对端会收到 PeerDisconnected。
    pub fn shutdown(&self) {
        self.swarm.shutdown();
    }
}
