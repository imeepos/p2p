//! [Node]：design §4 的业务 API 表面。

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{
    open_with_protocol, ProtocolHandler, ProtocolId, RequestResponse, RequestResponseClient,
    StreamFactory,
};
use p2p_swarm::{ConnectionGate, MetricsSnapshot, NodeEvent, Swarm, SwarmFactory};
use p2p_transport::TransportAddr;
use tokio::sync::broadcast;

use p2p_discovery::RendezvousClient;

use crate::rendezvous::parse_transport_addr;
use crate::{static_peers, NodeBuilder, NodeError};

pub struct Node {
    swarm: Arc<Swarm>,
    observation_addr: Option<SocketAddr>,
    /// rendezvous 客户端句柄（bootstrap 未接线为 None）：按需查号入口。
    rendezvous: Option<Arc<RendezvousClient>>,
    /// 静态对端登记文件句柄（未配置为 None）。
    static_peers: Option<Arc<static_peers::StaticPeersFile>>,
}

impl Node {
    pub(crate) fn new(
        swarm: Arc<Swarm>,
        observation_addr: Option<SocketAddr>,
        rendezvous: Option<Arc<RendezvousClient>>,
        static_peers: Option<Arc<static_peers::StaticPeersFile>>,
    ) -> Self {
        Self {
            swarm,
            observation_addr,
            rendezvous,
            static_peers,
        }
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

    /// 运行时指标快照（E5）：拨号各跳成败、重连次数、活跃连接/会话水位。
    pub fn metrics(&self) -> MetricsSnapshot {
        self.swarm.metrics()
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

    /// 挂断与该 peer 的连接（幂等）：返回是否确有在册连接被关闭。
    pub fn disconnect(&self, peer: &PeerId) -> bool {
        self.swarm.disconnect(peer)
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

    /// 按 PeerId 向 rendezvous 做一次精确查号（社交化发现 P1 原语）。
    /// bootstrap 未接线时显式报错（不静默返回空）；地址已按策略过滤。
    pub async fn query_peer(&self, peer: &str) -> Result<Vec<String>, NodeError> {
        let client = self
            .rendezvous
            .as_ref()
            .ok_or_else(|| NodeError::Assembly("rendezvous not wired: bootstrap empty".into()))?;
        let peer_id = parse_peer_id(peer)?;
        let addrs = client
            .query_peer(peer_id)
            .await
            .map_err(|e| NodeError::Assembly(e.to_string()))?;
        Ok(addrs.iter().map(ToString::to_string).collect())
    }

    /// 登记静态对端并落盘（社交化发现 P1 原语）：立即进入地址簿
    /// （Manual 来源，可拨），同时持久化到 static_peers_file。
    pub fn upsert_static_peer(
        &self,
        peer_id: &str,
        addrs: Vec<String>,
        note: &str,
    ) -> Result<(), NodeError> {
        let file = self
            .static_peers
            .as_ref()
            .ok_or_else(|| NodeError::Assembly("static_peers_file not configured".into()))?;
        let peer = parse_peer_id(peer_id)?;
        let parsed: Vec<TransportAddr> = addrs
            .iter()
            .map(|s| parse_transport_addr(s))
            .collect::<Result<_, _>>()?;
        file.upsert(peer_id.to_string(), addrs, note.to_string())?;
        self.swarm.add_peer_addresses(peer, parsed);
        Ok(())
    }

    /// 对端是否已在地址簿（含静态登记；观测/测试用）。
    pub fn peer_registered(&self, peer: &PeerId) -> bool {
        !self.swarm.peer_addrs(peer).is_empty()
    }

    /// 对端当前登记地址（观测/测试用）。
    pub fn peer_addrs(&self, peer: &PeerId) -> Vec<String> {
        self.swarm.peer_addrs(peer)
    }

    /// 本节点监听地址（0.0.0.0 已换成 127.0.0.1，可直接用于拨号/登记）。
    pub fn listen_addrs(&self) -> Vec<String> {
        self.swarm.listen_addr_strings()
    }

    /// 本节点观测反射器的绑定地址（未启用为 None）。
    pub fn observation_addr(&self) -> Option<SocketAddr> {
        self.observation_addr
    }

    /// 关停：停 accept 循环并断开全部连接；对端会收到 PeerDisconnected。
    pub fn shutdown(&self) {
        self.swarm.shutdown();
    }
}

/// base58 → PeerId；长度/编码错误显式上抛（与 CLI 解析同规则）。
pub(crate) fn parse_peer_id(s: &str) -> Result<PeerId, NodeError> {
    let bytes = bs58::decode(s)
        .into_vec()
        .map_err(|e| NodeError::Assembly(format!("bad peer id: {e}")));
    let bytes = bytes?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| NodeError::Assembly(format!("bad peer id length: {s}")))?;
    Ok(PeerId::from_bytes(arr))
}
