//! Swarm 核心：装配、公共 API 与事件总线（design §8/§9/§12）。

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};

use p2p_identity::{Keypair, PeerId};
use p2p_mux::{BoxedStream, MuxControl};
use p2p_protocol::{HandlerRegistry, ProtocolHandler, ProtocolId, StreamFactory};
use p2p_transport::{QuicTransport, TcpTransport, TransportAddr};
use tokio::sync::{broadcast, mpsc, watch};

use crate::pool::ConnectionPool;
use crate::{ConnectionGate, NodeEvent};

mod book;
mod config;
mod dial;
mod listen;
mod relay_session;
mod responder;

#[cfg(test)]
mod tests;

use book::AddressBook;
pub use book::{filter_loopback, AddrSource};
pub use config::SwarmConfig;
use config::{to_transport, EVENT_CAPACITY};
use dial::dial_peer;
use listen::spawn_accept_loops;
use relay_session::{spawn_sessions, RelayCmd};

/// 电路化/直连拨号共用的复用句柄别名。
pub(super) type Mux = Arc<dyn MuxControl>;

pub struct Swarm {
    keypair: Arc<Keypair>,
    dial_quic: QuicTransport,
    dial_tcp: TcpTransport,
    listen_addrs: Vec<TransportAddr>,
    advertised_addrs: Vec<TransportAddr>,
    observed_addrs: Mutex<Vec<TransportAddr>>,
    pool: Arc<ConnectionPool>,
    registry: RegistryCell,
    gate: Mutex<Option<Arc<dyn ConnectionGate>>>,
    address_book: Mutex<AddressBook>,
    events: broadcast::Sender<NodeEvent>,
    relay_sessions: Mutex<Vec<mpsc::Sender<RelayCmd>>>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl Swarm {
    /// 绑定 QUIC+TCP 监听并启动 accept/relay 会话；绑定失败原样上抛（装配期可见）。
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
            advertised_addrs: config.advertised_addrs,
            observed_addrs: Mutex::new(Vec::new()),
            pool: Arc::new(ConnectionPool::new()),
            registry: Arc::new(Mutex::new(config.registry)),
            gate: Mutex::new(None),
            address_book: Mutex::new(AddressBook::new()),
            events,
            relay_sessions: Mutex::new(Vec::new()),
            shutdown_tx,
            shutdown_rx,
        });
        spawn_accept_loops(&swarm, quic, tcp, tcp_listener);
        spawn_sessions(&swarm, config.relay_addrs);
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

    /// 幂等连接：池内已有连接直接复用，否则走降级链 直连→打洞→中继。
    pub async fn connect(&self, peer: PeerId) -> io::Result<()> {
        self.pool
            .get_or_dial(peer, dial_peer(self, peer))
            .await
            .map(|_| ())
    }

    /// 静态登记对端地址（显式配置，Manual 来源）；新地址发 PeerDiscovered。
    pub fn add_peer_addresses(&self, peer: PeerId, addrs: Vec<TransportAddr>) {
        self.add_peer_addresses_with_source(peer, addrs, AddrSource::Manual);
    }

    /// 按来源登记对端地址（发现转发入口）。mDNS 来源学习链路前缀：
    /// 同网段地址在直连跳排前（E3 同 NAT 排序缺陷）。
    pub fn add_peer_addresses_with_source(
        &self,
        peer: PeerId,
        addrs: Vec<TransportAddr>,
        source: AddrSource,
    ) {
        if addrs.is_empty() {
            return;
        }
        let (added, known) = {
            let mut book = self.address_book.lock().expect("addr lock");
            book.add(peer, addrs.into_iter().map(|addr| (addr, source)).collect())
        };
        if added {
            let strings = known.iter().map(ToString::to_string).collect();
            self.emit(NodeEvent::PeerDiscovered {
                peer,
                addrs: strings,
            });
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

    /// 注入地址观测学到的外部地址（design §7.2），打洞宣告观测优先。
    pub fn set_observed_addrs(&self, addrs: Vec<TransportAddr>) {
        *self.observed_addrs.lock().expect("observed lock") = addrs;
    }

    /// 打洞信令宣告的地址（design §7.2）：观测地址优先（跨网可拨），
    /// 其后为显式宣告或监听地址；去重并过滤 loopback。
    fn punch_addrs(&self) -> Vec<TransportAddr> {
        let mut out: Vec<TransportAddr> = Vec::new();
        let mut push_all = |addrs: &[TransportAddr]| {
            for addr in addrs {
                if !out.contains(addr) {
                    out.push(addr.clone());
                }
            }
        };
        push_all(&self.observed_addrs.lock().expect("observed lock"));
        if self.advertised_addrs.is_empty() {
            push_all(&self.listen_addrs);
        } else {
            push_all(&self.advertised_addrs);
        }
        filter_loopback(out)
    }

    fn punch_addrs_strs(&self) -> Vec<String> {
        self.punch_addrs().iter().map(ToString::to_string).collect()
    }

    /// 直连跳用地址：按来源/网段优先级排序（design §7.3 + E3）。
    fn addresses_of(&self, peer: PeerId) -> Vec<TransportAddr> {
        self.address_book
            .lock()
            .expect("addr lock")
            .sorted_addrs(&peer)
    }

    /// 开裸流（协议 ID 首帧由调用方写入）：按需取/建连接。
    pub async fn open_stream(
        &self,
        peer: &PeerId,
        _protocol: &ProtocolId,
    ) -> io::Result<BoxedStream> {
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

    /// 降级链 2/3 跳（打洞 + 中继电路）经由的会话命令入口。
    async fn relay_degrade(&self, peer: PeerId) -> io::Result<Mux> {
        let senders = self.relay_sessions.lock().expect("relay lock").clone();
        if senders.is_empty() {
            tracing::debug!(%peer, "relay fallback unavailable: no relay configured");
            return Err(io::Error::other("no relay configured"));
        }
        let mut last: Option<io::Error> = None;
        for tx in senders.iter() {
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            if tx
                .send(RelayCmd::Degrade {
                    peer,
                    reply: reply_tx,
                })
                .await
                .is_err()
            {
                last = Some(io::Error::other("relay session closed"));
                continue;
            }
            return match reply_rx.await {
                Ok(result) => result,
                Err(_) => Err(io::Error::other("relay session dropped the request")),
            };
        }
        Err(last.unwrap_or_else(|| io::Error::other("no relay session available")))
    }

    fn has_relay_sessions(&self) -> bool {
        !self.relay_sessions.lock().expect("relay lock").is_empty()
    }

    fn add_relay_session(&self, tx: mpsc::Sender<RelayCmd>) {
        self.relay_sessions.lock().expect("relay lock").push(tx);
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
