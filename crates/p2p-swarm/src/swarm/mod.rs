//! Swarm 核心：装配、公共 API 与事件总线（design §8/§9/§12）。

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

use p2p_identity::{Keypair, PeerId};
use p2p_mux::MuxControl;
use p2p_transport::{QuicTransport, TcpTransport, TransportAddr};
use tokio::sync::{broadcast, watch};

use crate::lifecycle::PeerLifecycleConfig;
use crate::liveness::{LivenessBook, LivenessSource};
use crate::pool::ConnectionPool;
use crate::usage::unix_now;
use crate::ConnectionGate;
use crate::NodeEvent;

mod book;
mod config;
mod degrade;
mod dial;
mod factory;
mod hangup;
mod lifecycle;
mod lifecycle_handlers;
mod lifecycle_task;
mod listen;
mod ping;
mod punch;
mod reclaim;
mod registry;
mod relay_degrade;
mod relay_selector;
mod relay_session;
mod responder;
mod serve;
mod streams;

pub use ping::PingHandler;

#[cfg(test)]
mod book_tests;
mod book_view;
#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod refresh_tests;
#[cfg(test)]
mod tests;

use crate::metrics::{Metrics, MetricsSnapshot};
use book::AddressBook;
mod refresh;
pub use book::{filter_loopback, AddrSource};
pub use config::SwarmConfig;
use config::{to_transport, EVENT_CAPACITY};
use dial::dial_peer;
use factory::RegistryCell;
pub use factory::SwarmFactory;
use lifecycle::LifecycleHandle;
use lifecycle::LifecycleMsg;
use listen::spawn_accept_loops;
pub use ping::PING_PROTOCOL;
pub use reclaim::ReclaimConfig;
use refresh::RefreshGate;
use relay_degrade::RelaySessionHandle;
use relay_selector::RelaySelectionCfg;
use relay_session::spawn_sessions;

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
    relay_sessions: Mutex<Vec<RelaySessionHandle>>,
    /// 上次降级选中的会话下标（滞回输入；越界自动作废）。
    last_relay_idx: AtomicUsize,
    /// 中继选择参数（门槛默认；配置入口留待需要时加法）。
    relay_selection_cfg: RelaySelectionCfg,
    metrics: Metrics,
    /// E6：对端连接生命周期监督句柄（状态机/探活/退避重连）。
    lifecycle: LifecycleHandle,
    /// E8：统一活跃度判定账本（观测面衍生，不驱动状态机，见 liveness.rs）。
    liveness: Arc<LivenessBook>,
    /// 重复发现重发门（refresh.rs）：地址无新增时按窗口限频重发 PeerDiscovered。
    refresh_gate: RefreshGate,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl Swarm {
    /// 绑定 QUIC+TCP 监听并启动 accept/relay 会话；绑定失败原样上抛（装配期可见）。
    /// 生命周期与回收参数取默认（见 [PeerLifecycleConfig]/[ReclaimConfig]）；
    /// 可配置入口见 [Self::start_with_lifecycle] / [Self::start_with_reclaim]
    /// （SwarmConfig 形状冻结，配置一律走加法入口）。
    pub async fn start(config: SwarmConfig) -> io::Result<Arc<Self>> {
        Self::assemble(
            config,
            PeerLifecycleConfig::default(),
            ReclaimConfig::default(),
        )
        .await
    }

    /// E6：指定生命周期参数的装配入口。其余语义同 [Self::start]。
    pub async fn start_with_lifecycle(
        config: SwarmConfig,
        lifecycle_cfg: PeerLifecycleConfig,
    ) -> io::Result<Arc<Self>> {
        Self::assemble(config, lifecycle_cfg, ReclaimConfig::default()).await
    }

    /// E8：指定空闲回收参数的装配入口（加法）。其余语义同 [Self::start]。
    pub async fn start_with_reclaim(
        config: SwarmConfig,
        lifecycle_cfg: PeerLifecycleConfig,
        reclaim_cfg: ReclaimConfig,
    ) -> io::Result<Arc<Self>> {
        Self::assemble(config, lifecycle_cfg, reclaim_cfg).await
    }

    async fn assemble(
        config: SwarmConfig,
        lifecycle_cfg: PeerLifecycleConfig,
        reclaim_cfg: ReclaimConfig,
    ) -> io::Result<Arc<Self>> {
        let bind = |port: u16| SocketAddr::new(IpAddr::from([0, 0, 0, 0]), port);
        let quic = QuicTransport::bind(bind(config.quic_port), &config.keypair).await?;
        let tcp = TcpTransport::new();
        let tcp_listener = tcp.bind(bind(config.tcp_port)).await?;
        let listen_addrs = vec![
            to_transport(quic.local_addr()?, true),
            to_transport(tcp_listener.local_addr()?, false),
        ];
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        // 内置 ping 应答注册（E6 探活的应答侧）；用户已注册时不抢占
        let registry = ping::registry_with_ping(config.registry.clone());
        let (lifecycle_events, _) = broadcast::channel(EVENT_CAPACITY);
        let (lifecycle, lifecycle_rx) =
            LifecycleHandle::new(lifecycle_cfg, lifecycle_events.clone());
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let swarm = Arc::new(Self {
            keypair: config.keypair,
            dial_quic: QuicTransport::new()?,
            dial_tcp: TcpTransport::new(),
            listen_addrs,
            advertised_addrs: config.advertised_addrs,
            observed_addrs: Mutex::new(Vec::new()),
            pool: Arc::new(ConnectionPool::new()),
            registry: Arc::new(Mutex::new(registry)),
            gate: Mutex::new(None),
            address_book: Mutex::new(AddressBook::new()),
            events,
            relay_sessions: Mutex::new(Vec::new()),
            last_relay_idx: AtomicUsize::new(0),
            relay_selection_cfg: RelaySelectionCfg::default(),
            metrics: Metrics::default(),
            lifecycle,
            liveness: Arc::new(LivenessBook::new(lifecycle_events.clone())),
            refresh_gate: RefreshGate::default(),
            shutdown_tx,
            shutdown_rx,
        });
        spawn_accept_loops(&swarm, quic, tcp, tcp_listener);
        spawn_sessions(&swarm, config.relay_addrs);
        lifecycle_task::start_supervisor(&swarm, lifecycle_rx);
        reclaim::spawn_reclaim(&swarm, &reclaim_cfg);
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

    /// 运行时指标快照（E5）：拨号各跳成败、重连次数、活跃连接/会话水位。
    pub fn metrics(&self) -> MetricsSnapshot {
        let conns = self.pool.len() as u64;
        let sessions = self.relay_sessions.lock().expect("relay lock").len() as u64;
        self.metrics.snapshot(conns, sessions)
    }

    /// 幂等连接：池内已有连接直接复用，否则走降级链 直连→打洞→中继。
    /// E6 钩子：未跟踪 peer 的首拨建档（Disconnected→Connecting），
    /// 失败回报监督者出册（从未连上的 peer 不自动重连）。
    pub async fn connect(&self, peer: PeerId) -> io::Result<()> {
        if self.pool.get(&peer).is_none() {
            self.lifecycle.notify(LifecycleMsg::DialStart { peer });
        }
        self.pool
            .get_or_dial(peer, dial_peer(self, peer))
            .await
            .inspect_err(|_| self.lifecycle.notify(LifecycleMsg::DialFailed { peer }))
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
        let (added, known, aggregate) = {
            let mut book = self.address_book.lock().expect("addr lock");
            book.add(peer, addrs.into_iter().map(|addr| (addr, source)).collect())
        };
        if source != AddrSource::Manual {
            // 发现刷新 = 对端在网上的正信号（TTL 续期）；Manual 是静态登记，
            // 无 TTL 语义，不构成活跃证据。地址无新增的重复发现同样刷新——
            // 否则在线但对端地址不变的节点在活跃度账本里被漏记。
            self.liveness
                .note_alive(peer, LivenessSource::Discovery, unix_now());
        }
        // 重复发现限频重发（refresh.rs）：地址去重后本无事件，但上层
        // lastSeen 只认 PeerDiscovered，每窗口放行一条把「最后活跃」推进。
        // gate 总是过账（含首见），否则首见后的紧邻重复发现会被误放行。
        let gated = source != AddrSource::Manual
            && self.refresh_gate.allows(peer, std::time::Instant::now());
        if added || gated {
            let strings = known.iter().map(ToString::to_string).collect();
            self.emit(NodeEvent::PeerDiscovered {
                peer,
                addrs: strings,
                source: aggregate,
            });
        }
    }

    /// 发现条目过期（design §7.1：TTL 内未刷新即判离线）。池内有活连接时
    /// 不得谎报断开：缓存过期只是发现面失联，连接面仍在（「拨通即闪断」根因之一）。
    /// 活跃度判定同样以连接面在场为豁免：活连接持续被探活证实，发现面过期
    /// 不足以判死（判死条件与 PeerDisconnected 一致，见 liveness.rs 注释）。
    pub fn on_peer_expired(&self, peer: PeerId) {
        if self.pool.get(&peer).is_some() {
            tracing::debug!(%peer, "discovery entry expired but connection alive, keep it");
            return;
        }
        self.liveness
            .note_dead(peer, LivenessSource::Discovery, unix_now());
        self.emit(NodeEvent::PeerDisconnected { peer });
    }

    fn is_stopping(&self) -> bool {
        *self.shutdown_rx.borrow()
    }

    /// 注入地址观测学到的外部地址（design §7.2），打洞宣告观测优先。
    pub fn set_observed_addrs(&self, addrs: Vec<TransportAddr>) {
        *self.observed_addrs.lock().expect("observed lock") = addrs;
    }

    /// 直连跳用地址：按来源/网段优先级排序，hairpin 候选同级殿后（design §7.3 + E3/E4）。
    /// 返回 (地址, 是否 hairpin 候选)。
    fn addresses_of(&self, peer: PeerId) -> Vec<(TransportAddr, bool)> {
        let observed = self.observed_addrs.lock().expect("observed lock").clone();
        self.address_book
            .lock()
            .expect("addr lock")
            .sorted_addrs(&peer, &observed)
    }

    /// 无订阅者时丢弃属正常态，不算失败路径。
    fn emit(&self, event: NodeEvent) {
        let _ = self.events.send(event);
    }
}
