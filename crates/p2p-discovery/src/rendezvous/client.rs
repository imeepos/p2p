//! rendezvous 客户端：周期签名注册 + 查询 + last-known-good 缓存，事件推入统一 channel。

use std::sync::Arc;
use std::time::{Duration, Instant};

use p2p_identity::{Keypair, PeerId};
use p2p_transport::TransportAddr;
use tokio::sync::mpsc;

use crate::cache::MemCache;
use crate::AddrCache;
use crate::rendezvous::link::{RendezvousConn, RendezvousError, RendezvousLink};
use crate::rendezvous::messages::{sign_register, AddrMsg, Request, Response};
use crate::{DiscoveredPeer, Discovery, DiscoveryEvent, Source};

/// rendezvous 客户端配置。
pub struct RendezvousConfig {
    pub namespace: String,
    pub keypair: Arc<Keypair>,
    /// 连接缝：真实传输由编排会话实现，测试用 duplex mock。
    pub link: Arc<dyn RendezvousLink>,
    /// 本机上报的监听地址。
    pub addrs: Vec<TransportAddr>,
    pub ttl_secs: u32,
    /// 注册刷新间隔，默认 ttl 的一半。
    pub register_interval: Duration,
    /// 查询间隔，默认 5s。
    pub query_interval: Duration,
}

impl RendezvousConfig {
    pub fn new(namespace: impl Into<String>, keypair: Keypair, link: Arc<dyn RendezvousLink>) -> Self {
        Self {
            namespace: namespace.into(),
            keypair: Arc::new(keypair),
            link,
            addrs: Vec::new(),
            ttl_secs: 60,
            register_interval: Duration::from_secs(30),
            query_interval: Duration::from_secs(5),
        }
    }
}

/// rendezvous 发现源：以独立任务运行，周期注册/查询，失败发 Failed 事件。
pub struct RendezvousClient {
    config: RendezvousConfig,
}

impl RendezvousClient {
    pub fn new(config: RendezvousConfig) -> Self {
        Self { config }
    }

    /// 签名注册本机地址；注册失败即让上层走 Failed 事件 + 重连。
    async fn register(&self, conn: &mut RendezvousConn) -> Result<(), RendezvousError> {
        let reg = sign_register(
            &self.config.keypair,
            &self.config.namespace,
            &self.config.addrs,
            self.config.ttl_secs,
        );
        let resp = conn.roundtrip(Request::register(reg)).await?;
        resp.ensure_ok().map_err(RendezvousError::Protocol)
    }

    /// 查询整个 namespace，把应答映射为 Discovered 事件并写入 last-known-good 缓存。
    async fn query_and_emit(
        &self,
        conn: &mut RendezvousConn,
        events: &mpsc::Sender<DiscoveryEvent>,
        cache: &MemCache,
    ) -> Result<(), RendezvousError> {
        let req = Request::query(self.config.namespace.clone(), Vec::new());
        let resp = conn.roundtrip(req).await?;
        resp.ensure_ok().map_err(RendezvousError::Protocol)?;
        let ttl = Duration::from_secs(self.config.ttl_secs.into());
        for (peer, addrs) in response_to_peers(&resp) {
            if peer == self.config.keypair.peer_id() {
                continue;
            }
            cache.put(peer, addrs.clone(), ttl);
            let ev = DiscoveryEvent::Discovered(DiscoveredPeer {
                peer,
                addrs,
                source: Source::Rendezvous,
                expires_at: Some(Instant::now() + ttl),
            });
            if events.send(ev).await.is_err() {
                return Err(RendezvousError::Send("event channel closed".into()));
            }
        }
        Ok(())
    }

    /// 一条连接上的注册/查询循环：断连即返回，由 run 重连。
    async fn connect_and_loop(
        &self,
        events: &mpsc::Sender<DiscoveryEvent>,
        cache: &MemCache,
    ) -> Result<(), RendezvousError> {
        let mut conn = self.config.link.connect().await?;
        self.register(&mut conn).await?;
        self.query_and_emit(&mut conn, events, cache).await?;

        let reg_tick = tokio::time::interval_at(
            tokio::time::Instant::now() + self.config.register_interval,
            self.config.register_interval,
        );
        let query_tick = tokio::time::interval_at(
            tokio::time::Instant::now() + self.config.query_interval,
            self.config.query_interval,
        );
        tokio::pin!(reg_tick);
        tokio::pin!(query_tick);
        loop {
            tokio::select! {
                _ = reg_tick.tick() => self.register(&mut conn).await?,
                _ = query_tick.tick() => self.query_and_emit(&mut conn, events, cache).await?,
            }
        }
    }
}

/// 把服务端应答解析为 (PeerId, 地址列表)；peer_id 非法或地址为空即跳过。
pub(crate) fn response_to_peers(resp: &Response) -> Vec<(PeerId, Vec<TransportAddr>)> {
    resp.peers
        .iter()
        .filter_map(|entry| {
            let peer_bytes = <[u8; 32]>::try_from(entry.peer_id.as_slice()).ok()?;
            let addrs: Vec<TransportAddr> = entry.addrs.iter().filter_map(AddrMsg::to_addr).collect();
            (!addrs.is_empty()).then_some((PeerId::from_bytes(peer_bytes), addrs))
        })
        .collect()
}

#[async_trait::async_trait]
impl Discovery for RendezvousClient {
    fn name(&self) -> &'static str {
        "rendezvous"
    }

    async fn run(self: Arc<Self>, events: mpsc::Sender<DiscoveryEvent>) {
        let cache = MemCache::new();
        let mut backoff = Duration::from_millis(500);
        loop {
            match self.connect_and_loop(&events, &cache).await {
                Ok(()) => tracing::debug!(
                    target: "p2p_discovery",
                    "rendezvous connection ended, reconnecting"
                ),
                Err(err) => {
                    let _ = events
                        .send(DiscoveryEvent::Failed {
                            source: Source::Rendezvous,
                            reason: err.to_string(),
                        })
                        .await;
                }
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(30));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendezvous::link::mock::{MockLink, conn_from_duplex};
    use crate::rendezvous::messages::PeerEntry;
    use crate::rendezvous::server::{RendezvousRegistry, serve_link};

    fn sample_addrs() -> Vec<TransportAddr> {
        vec![TransportAddr::Quic { ip: "10.0.0.5".parse().unwrap(), port: 4000 }]
    }

    #[test]
    fn response_maps_to_peers() {
        let kp = Keypair::generate();
        let addrs = sample_addrs();
        let resp = Response {
            error: String::new(),
            peers: vec![PeerEntry {
                peer_id: kp.peer_id().as_bytes().to_vec(),
                addrs: addrs.iter().map(AddrMsg::from_addr).collect(),
            }],
        };
        let peers = response_to_peers(&resp);
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].0, kp.peer_id());
        assert_eq!(peers[0].1, addrs);
    }

    #[test]
    fn response_skips_bad_entries() {
        let resp = Response {
            error: String::new(),
            peers: vec![PeerEntry { peer_id: vec![1, 2, 3], addrs: Vec::new() }],
        };
        assert!(response_to_peers(&resp).is_empty());
    }

    #[tokio::test]
    async fn client_registers_and_discovers_other_peer() {
        let (client_side, server_side) = tokio::io::duplex(4096);
        let link: Arc<dyn RendezvousLink> = Arc::new(MockLink::new(client_side));
        let client = RendezvousClient::new(RendezvousConfig::new("room-a", Keypair::generate(), link));

        let registry = Arc::new(RendezvousRegistry::new());
        let server_registry = registry.clone();
        let server_task = tokio::spawn(async move {
            let mut server = conn_from_duplex(server_side);
            let _ = serve_link(&mut server, &server_registry).await;
        });

        // 另一节点直接注册，模拟其已上线
        let other = Keypair::generate();
        let other_addrs = vec![TransportAddr::Quic { ip: "10.0.0.9".parse().unwrap(), port: 9000 }];
        let reg = sign_register(&other, "room-a", &other_addrs, 60);
        registry.register(&reg).expect("other registered");

        let (tx, mut rx) = mpsc::channel(16);
        let cache = MemCache::new();
        let mut conn = client.config.link.connect().await.expect("connect");
        client.register(&mut conn).await.expect("register");
        client.query_and_emit(&mut conn, &tx, &cache).await.expect("query");

        match rx.recv().await {
            Some(DiscoveryEvent::Discovered(dp)) => {
                assert_eq!(dp.peer, other.peer_id());
                assert_eq!(dp.source, Source::Rendezvous);
                assert_eq!(dp.addrs, other_addrs);
                assert_eq!(cache.get(&other.peer_id()), Some(other_addrs));
            }
            other_ev => panic!("expected Discovered, got {other_ev:?}"),
        }
        server_task.abort();
    }
}
