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
use crate::rendezvous::{parse_transport_addr, RendezvousServer, TransportLink};
use crate::{NodeConfig, NodeError};

/// rendezvous 默认命名空间（design §4：唯一分组原语，业务含义由业务层定义）。
const DEFAULT_NAMESPACE: &str = "p2p-base";

/// 发现事件通道容量。
const DISCOVERY_EVENTS: usize = 64;

pub(crate) async fn build(cfg: NodeConfig) -> Result<Node, NodeError> {
    let keypair = Arc::new(load_identity(&cfg.data_dir)?);
    let swarm = Swarm::start(SwarmConfig {
        keypair: keypair.clone(),
        quic_port: cfg.quic_port,
        tcp_port: cfg.tcp_port,
        registry: Arc::new(HandlerRegistry::default()),
    })
    .await?;

    // 底座自身能力与业务协议同机制注册（design §2 dogfooding）
    swarm.register(Arc::new(RendezvousServer::new()));

    let listen_addrs = swarm.listen_addrs();
    spawn_discovery(&cfg, keypair, swarm.clone(), &listen_addrs)?;

    Ok(Node::new(swarm))
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
    listen_addrs: &[TransportAddr],
) -> Result<(), NodeError> {
    let (tx, rx) = mpsc::channel(DISCOVERY_EVENTS);
    if cfg.enable_mdns {
        let (quic_port, tcp_port) = listen_ports(listen_addrs);
        let mut mdns_cfg = MdnsConfig::new(keypair.peer_id());
        mdns_cfg.quic_port = quic_port;
        mdns_cfg.tcp_port = tcp_port;
        tokio::spawn(Arc::new(MdnsDiscovery::new(mdns_cfg)).run(tx.clone()));
        tracing::info!(?quic_port, ?tcp_port, "mdns discovery enabled");
    }
    match wire_rendezvous(cfg, &keypair, listen_addrs)? {
        Some(client) => {
            tokio::spawn(client.run(tx));
            tracing::info!(bootstrap = ?cfg.bootstrap, "rendezvous wired");
        }
        None => tracing::info!("bootstrap empty, rendezvous wiring skipped"),
    }
    tokio::spawn(forward_discovery(rx, swarm));
    Ok(())
}

fn wire_rendezvous(
    cfg: &NodeConfig,
    keypair: &Arc<Keypair>,
    listen_addrs: &[TransportAddr],
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
    rcfg.addrs = listen_addrs.to_vec();
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