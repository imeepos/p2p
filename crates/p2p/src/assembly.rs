//! build() 装配（design §11.1 启动序）：身份 → 监听 → swarm → 发现接线。

use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;

use p2p_discovery::{Discovery, MdnsConfig, MdnsDiscovery, RendezvousClient, RendezvousConfig};
use p2p_identity::Keypair;
use p2p_protocol::HandlerRegistry;
use p2p_swarm::{Swarm, SwarmConfig};
use p2p_transport::TransportAddr;
use tokio::sync::mpsc;

use crate::discovery::forward_discovery;
use crate::node::Node;
use crate::observe;
use crate::rendezvous::{parse_transport_addr, RendezvousServer, TransportLink};
use crate::{node::parse_peer_id, static_peers};
use crate::{NodeConfig, NodeError};

/// rendezvous 默认命名空间（design §4：唯一分组原语，业务含义由业务层定义）。
const DEFAULT_NAMESPACE: &str = "p2p-base";

/// 发现事件通道容量。
const DISCOVERY_EVENTS: usize = 64;

pub(crate) async fn build(cfg: NodeConfig) -> Result<Node, NodeError> {
    let keypair = Arc::new(load_identity(&cfg.data_dir)?);

    // 观测反射器（bootstrap 角色节点）：独立 UDP 口，回答对端观测地址
    let mut reflector_addr = None;
    if let Some(port) = cfg.observation_port {
        let local = observe::spawn_reflector(port)
            .await
            .map_err(|e| NodeError::Assembly(format!("observation reflector bind failed: {e}")))?;
        tracing::info!(%local, "observation reflector enabled");
        reflector_addr = Some(local);
    }
    // 地址观测（design §7.2）：学习自身公网映射地址（先于注册与打洞宣告）
    let observed = observe::observe_external_addrs(&cfg.observation_addrs).await;
    // 失败路径留观测信号（E5 复盘）：观测全失败意味着 rendezvous 注册将回退监听
    // 地址——loopback 对远端不可拨，跨网发现/被拨全部失效，必须让运维可见
    if !cfg.observation_addrs.is_empty() && observed.is_empty() {
        tracing::warn!(
            targets = ?cfg.observation_addrs,
            "address observation failed on all targets; rendezvous registration falls back to listen addrs (loopback addrs are undialable from other machines)"
        );
    }

    let relay_addrs = parse_all(&cfg.relay_addrs)?;
    let advertised_addrs = parse_all(&cfg.advertised_addrs)?;
    let swarm = Swarm::start(SwarmConfig {
        keypair: keypair.clone(),
        quic_port: cfg.quic_port,
        tcp_port: cfg.tcp_port,
        registry: Arc::new(HandlerRegistry::default()),
        relay_addrs,
        advertised_addrs,
    })
    .await?;

    // 底座自身能力与业务协议同机制注册（design §2 dogfooding）；
    // 公共部署（rendezvous_public_only）拒收全不可路由注册（E5 地址卫生）
    swarm.register(Arc::new(RendezvousServer::with_public_only(
        cfg.rendezvous_public_only,
    )?));

    let listen_addrs = swarm.listen_addrs();
    let observed_addrs = observe::observed_transport_addrs(&observed, &listen_addrs);
    swarm.set_observed_addrs(observed_addrs);
    let reg_addrs = observe::merge_observed_with_listen(observed.first().copied(), &listen_addrs);
    // 注册集无一条可路由地址：跨网节点无法拨到本机（E5 复盘的泄漏前兆），启动即告警
    if !cfg.bootstrap.is_empty() && !reg_addrs.iter().any(TransportAddr::is_routable) {
        tracing::warn!(
            addrs = ?reg_addrs,
            "registering no routable addr to rendezvous; peers on other machines cannot dial this node"
        );
    }

    let rendezvous = spawn_discovery(&cfg, keypair, swarm.clone(), &reg_addrs, &listen_addrs)?;

    let static_peers = match &cfg.static_peers_file {
        Some(path) => {
            let file = static_peers::StaticPeersFile::load(path.clone()).map_err(NodeError::Io)?;
            for entry in file.entries() {
                register_static_entry(&swarm, &entry);
            }
            Some(file)
        }
        None => None,
    };

    Ok(Node::new(
        swarm,
        reflector_addr,
        rendezvous,
        static_peers.map(Arc::new),
    ))
}

/// 登记一条静态对端；坏条目 warn 跳过不拖垮启动（数据文件可人工编辑）。
fn register_static_entry(swarm: &Swarm, entry: &static_peers::StaticPeerEntry) {
    let parsed = parse_peer_id(&entry.peer_id)
        .and_then(|peer| parse_all(&entry.addrs).map(|addrs| (peer, addrs)));
    match parsed {
        Ok((peer, addrs)) => swarm.add_peer_addresses(peer, addrs),
        Err(e) => tracing::warn!(
            peer_id = %entry.peer_id,
            error = %e,
            "static peer entry skipped"
        ),
    }
}

/// 批量解析传输地址；任一非法即装配失败（显式配置错误必须响亮）。
fn parse_all(addrs: &[String]) -> Result<Vec<TransportAddr>, NodeError> {
    addrs.iter().map(|s| parse_transport_addr(s)).collect()
}

/// 身份持久化：目录 0700，种子文件 0600，重启身份不变（design §6）。
fn load_identity(data_dir: &Path) -> io::Result<Keypair> {
    create_private_dir(data_dir)?;
    p2p_identity::load_or_generate_seed(&data_dir.join("key.seed"))
}

fn create_private_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

/// 接线发现源：mdns 开关 + rendezvous（bootstrap 为空跳过并留日志），事件统一转发进 swarm。
fn spawn_discovery(
    cfg: &NodeConfig,
    keypair: Arc<Keypair>,
    swarm: Arc<Swarm>,
    reg_addrs: &[TransportAddr],
    listen_addrs: &[TransportAddr],
) -> Result<Option<Arc<RendezvousClient>>, NodeError> {
    let (tx, rx) = mpsc::channel(DISCOVERY_EVENTS);
    if cfg.enable_mdns {
        let (quic_port, tcp_port) = listen_ports(listen_addrs);
        let mut mdns_cfg = MdnsConfig::new(keypair.peer_id());
        mdns_cfg.quic_port = quic_port;
        mdns_cfg.tcp_port = tcp_port;
        tokio::spawn(Arc::new(MdnsDiscovery::new(mdns_cfg)).run(tx.clone()));
        tracing::info!(?quic_port, ?tcp_port, "mdns discovery enabled");
    }
    let rendezvous = match wire_rendezvous(cfg, &keypair, reg_addrs)? {
        Some(client) => {
            tokio::spawn(client.clone().run(tx));
            tracing::info!(bootstrap = ?cfg.bootstrap, "rendezvous wired");
            Some(client)
        }
        None => {
            tracing::info!("bootstrap empty, rendezvous wiring skipped");
            None
        }
    };
    tokio::spawn(forward_discovery(rx, swarm));
    Ok(rendezvous)
}

fn wire_rendezvous(
    cfg: &NodeConfig,
    keypair: &Arc<Keypair>,
    reg_addrs: &[TransportAddr],
) -> Result<Option<Arc<RendezvousClient>>, NodeError> {
    if cfg.bootstrap.is_empty() {
        return Ok(None);
    }
    let mut addrs = Vec::with_capacity(cfg.bootstrap.len());
    for s in &cfg.bootstrap {
        addrs.push(parse_transport_addr(s)?);
    }
    let link = Arc::new(TransportLink::new(addrs, keypair.clone())?);
    let mut rcfg = RendezvousConfig::new(DEFAULT_NAMESPACE, (**keypair).clone(), link);
    rcfg.addrs = reg_addrs.to_vec();
    // 信任域（E5）：rendezvous 本体在同机（bootstrap 全 loopback）时关闭查询侧
    // 过滤，保留同机全 loopback 注册的可发现性；跨网 rendezvous 一律过滤
    let same_machine = cfg.bootstrap.iter().all(|s| {
        parse_transport_addr(s)
            .map(|a| !a.is_routable())
            .unwrap_or(false)
    });
    rcfg.strip_unroutable = !same_machine;
    Ok(Some(Arc::new(RendezvousClient::new(rcfg))))
}

fn listen_ports(addrs: &[TransportAddr]) -> (Option<u16>, Option<u16>) {
    let mut quic = None;
    let mut tcp = None;
    for addr in addrs {
        match addr {
            TransportAddr::Quic { port, .. } => quic = Some(*port),
            TransportAddr::Tcp { port, .. } => tcp = Some(*port),
        }
    }
    (quic, tcp)
}
