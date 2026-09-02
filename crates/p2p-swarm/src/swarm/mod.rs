//! Swarm 核心：装配、公共 API 与事件总线（design §8/§9/§12）。

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};

use p2p_identity::{Keypair, PeerId};
use p2p_mux::BoxedStream;
use p2p_protocol::{HandlerRegistry, ProtocolHandler, ProtocolId, StreamFactory};
use p2p_transport::{QuicTransport, TcpTransport, TransportAddr};
use tokio::sync::{broadcast, watch};

use crate::pool::ConnectionPool;
use crate::{ConnectionGate, NodeEvent};

mod dial;
mod listen;

use dial::dial_peer;
use listen::spawn_accept_loops;

/// broadcast 事件通道容量；慢消费者落后超过即 Lagged，由其自行处理。
const EVENT_CAPACITY: usize = 128;

/// Swarm 装配参数。
pub struct SwarmConfig {
    pub keypair: Arc<Keypair>,
    /// 0 = 随机端口。
    pub quic_port: u16,
    pub tcp_port: u16,
    pub registry: Arc<HandlerRegistry>,
}

pub struct Swarm {
    keypair: Arc<Keypair>,
    dial_quic: QuicTransport,
    dial_tcp: TcpTransport,
    listen_addrs: Vec<TransportAddr>,
    pool: Arc<ConnectionPool>,
    registry: RegistryCell,
    gate: Mutex<Option<Arc<dyn ConnectionGate>>>,
    address_book: Mutex<HashMap<PeerId, Vec<TransportAddr>>>,
    events: broadcast::Sender<NodeEvent>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl Swarm {
    /// 绑定 QUIC+TCP 监听并启动 accept 循环；绑定失败原样上抛（装配期可见）。
    pub async fn start(config: SwarmConfig) -> io::Result<Arc<Self>> {
        let bind = |port: u16| SocketAddr::new(IpAddr::from([0, 0, 0, 0]), port);
        let quic = QuicTransport::bind(bind(config.quic_port), &config.keypair).await?;
        let tcp = TcpTransport::new();
        let tcp_listener = tcp.bind(bind(config.tcp_port)).await?;
        let listen_addrs = vec![
            to_transport(quic.local_addr()?, true),
            to_transport(tcp_listener.local_addr()?, false),
        ];
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let swarm = Arc::new(Self {
            keypair: config.keypair,
            dial_quic: QuicTransport::new()?,
            dial_tcp: TcpTransport::new(),
            listen_addrs,
            pool: Arc::new(ConnectionPool::new()),
            registry: Arc::new(Mutex::new(config.registry)),
            gate: Mutex::new(None),
            address_book: Mutex::new(HashMap::new()),
            events,
            shutdown_tx,
            shutdown_rx,
        });
        spawn_accept_loops(&swarm, quic, tcp, tcp_listener);
        Ok(swarm)
    }

    pub fn local_peer_id(&self) -> PeerId {
        self.keypair.peer_id()
    }

    /// 已绑定监听地址；未指定 IP（0.0.0.0）替换为 127.0.0.1 保证可拨。
    pub fn listen_addrs(&self) -> Vec<TransportAddr> {
        self.listen_addrs.clone()
    }

    pub fn listen_addr_strings(&self) -> Vec<String> {
        self.listen_addrs.iter().map(ToString::to_string).collect()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<NodeEvent> {
        self.events.subscribe()
    }

    /// 注册协议 handler：复制-改-换，进行中的分发继续使用旧快照。
    pub fn register(&self, handler: Arc<dyn ProtocolHandler>) {
        let mut guard = self.registry.lock().expect("registry lock");
        let mut next = HandlerRegistry::default();
        for id in guard.protocols() {
            if let Some(h) = guard.get(&id) {
                next.register(h);
            }
        }
        next.register(handler);
        *guard = Arc::new(next);
    }

    pub fn set_gate(&self, gate: Arc<dyn ConnectionGate>) {
        *self.gate.lock().expect("gate lock") = Some(gate);
    }

    /// 幂等连接：池内已有连接直接复用，否则按地址簿顺序直连。
    pub async fn connect(&self, peer: PeerId) -> io::Result<()> {
        self.pool
            .get_or_dial(peer, dial_peer(self, peer))
            .await
            .map(|_| ())
    }

    /// 静态登记对端地址（发现事件与显式配置共用入口）；新地址发 PeerDiscovered。
    pub fn add_peer_addresses(&self, peer: PeerId, addrs: Vec<TransportAddr>) {
        if addrs.is_empty() {
            return;
        }
        let (added, known) = {
            let mut book = self.address_book.lock().expect("addr lock");
            let entry = book.entry(peer).or_default();
            let before = entry.len();
            for addr in addrs {
                if !entry.contains(&addr) {
                    entry.push(addr);
                }
            }
            (entry.len() > before, entry.clone())
        };
        if added {
            let strings = known.iter().map(ToString::to_string).collect();
            self.emit(NodeEvent::PeerDiscovered { peer, addrs: strings });
        }
    }

    /// 发现条目过期（design §7.1：TTL 内未刷新即判离线），发断开事件。
    pub fn on_peer_expired(&self, peer: PeerId) {
        tracing::debug!(%peer, "discovery entry expired");
        self.emit(NodeEvent::PeerDisconnected { peer });
    }

    /// 关停：停 accept 循环并断开全部在册连接；serve 循环退出时各自发断开事件。
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
        for peer in self.pool.clear() {
            tracing::debug!(%peer, "connection dropped on shutdown");
        }
    }

    fn is_stopping(&self) -> bool {
        *self.shutdown_rx.borrow()
    }

    fn addresses_of(&self, peer: PeerId) -> Vec<TransportAddr> {
        self.address_book
            .lock()
            .expect("addr lock")
            .get(&peer)
            .cloned()
            .unwrap_or_default()
    }

    /// 开裸流（协议 ID 首帧由调用方写入）：按需取/建连接。
    pub async fn open_stream(&self, peer: &PeerId, _protocol: &ProtocolId) -> io::Result<BoxedStream> {
        let mux = self.pool.get_or_dial(*peer, dial_peer(self, *peer)).await?;
        mux.open_stream().await
    }

    /// 门禁裁决：未配置即放行；锁外 await，不阻塞注册路径。
    async fn gate_allows(&self, peer: PeerId) -> bool {
        let gate = self.gate.lock().expect("gate lock").clone();
        match gate {
            Some(g) => g.allow(&peer).await,
            None => true,
        }
    }

    /// 无订阅者时丢弃属正常态，不算失败路径。
    fn emit(&self, event: NodeEvent) {
        let _ = self.events.send(event);
    }
}

/// 未指定 IP（0.0.0.0/::）替换为 127.0.0.1，保证地址簿里的监听地址可直连。
fn to_transport(addr: SocketAddr, quic: bool) -> TransportAddr {
    let ip = if addr.ip().is_unspecified() {
        IpAddr::from([127, 0, 0, 1])
    } else {
        addr.ip()
    };
    if quic {
        TransportAddr::Quic { ip, port: addr.port() }
    } else {
        TransportAddr::Tcp { ip, port: addr.port() }
    }
}

/// handler 注册表的共享单元：注册为复制-改-换，分发按快照路由。
pub type RegistryCell = Arc<Mutex<Arc<HandlerRegistry>>>;

/// [Swarm] 的可拥有工厂句柄：RequestResponseClient 等需要持有工厂的场景使用。
#[derive(Clone)]
pub struct SwarmFactory(pub Arc<Swarm>);

#[async_trait::async_trait]
impl StreamFactory for SwarmFactory {
    async fn open_stream(&self, peer: &PeerId, protocol: &ProtocolId) -> io::Result<BoxedStream> {
        self.0.open_stream(peer, protocol).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// 直连按地址顺序尝试：首地址拨号失败必须换下一地址成功，
    /// 且失败地址发 DialFailed（design §12 失败路径可见）。
    #[tokio::test]
    async fn dial_falls_through_failed_addr() {
        let registry_of = || Arc::new(HandlerRegistry::default());
        let swarm = Swarm::start(SwarmConfig {
            keypair: Arc::new(Keypair::generate()),
            quic_port: 0,
            tcp_port: 0,
            registry: registry_of(),
        })
        .await
        .expect("bind swarm");
        let helper = Swarm::start(SwarmConfig {
            keypair: Arc::new(Keypair::generate()),
            quic_port: 0,
            tcp_port: 0,
            registry: registry_of(),
        })
        .await
        .expect("bind helper");

        let helper_peer = helper.local_peer_id();
        let tcp_addr = helper
            .listen_addrs()
            .into_iter()
            .find(|a| matches!(a, TransportAddr::Tcp { .. }))
            .expect("helper tcp addr");
        // 首地址用本机未监听的 TCP 端口：loopback 拒绝即时返回（UDP 拒绝在 macOS 上要等 30s 超时）
        swarm.add_peer_addresses(
            helper_peer,
            vec![
                TransportAddr::Tcp { ip: IpAddr::from([127, 0, 0, 1]), port: 1 },
                tcp_addr,
            ],
        );

        let mut events = swarm.subscribe();
        swarm.connect(helper_peer).await.expect("dial via fallback");

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut saw_failed = false;
        loop {
            match tokio::time::timeout_at(deadline, events.recv()).await {
                Ok(Ok(NodeEvent::DialFailed { .. })) => {
                    saw_failed = true;
                    break;
                }
                Ok(Ok(_)) => continue,
                _ => break,
            }
        }
        assert!(saw_failed, "failed first addr must emit DialFailed");
    }
}