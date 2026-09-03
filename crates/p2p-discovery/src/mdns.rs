//! mDNS 局域网发现（design §7.1）：通告本机 + 浏览局域网，事件推入统一 channel。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use p2p_identity::PeerId;
use p2p_transport::TransportAddr;
use tokio::sync::mpsc;

use self::txt::{decode_txt, encode_txt};
use crate::{DiscoveredPeer, Discovery, DiscoveryEvent, Source};

/// 服务类型：局域网上所有 p2p-base 节点共用。
/// mdns-sd 0.13 要求以带尾点的 '._udp.local.' 结尾（register/browse 共用，防 E1 阻断回归）。
pub const SERVICE_TYPE: &str = "_p2pbase._udp.local.";

/// 过期扫描周期：比通告 TTL 小一个量级即可。
const SCAN_INTERVAL: Duration = Duration::from_secs(1);

/// mDNS 发现配置。
pub struct MdnsConfig {
    pub service_type: String,
    /// 实例名：同机多实例需唯一，避免服务名冲突。
    pub instance: String,
    pub peer_id: PeerId,
    pub quic_port: Option<u16>,
    pub tcp_port: Option<u16>,
    /// 周期通告间隔，默认 5s。
    pub announce_interval: Duration,
    /// 通告 TTL 秒数，远端据此判定离线。
    pub ttl_secs: u32,
}

impl MdnsConfig {
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            service_type: SERVICE_TYPE.to_string(),
            instance: format!("p2p-{}", &peer_id.to_string()[..8]),
            peer_id,
            quic_port: None,
            tcp_port: None,
            announce_interval: Duration::from_secs(5),
            ttl_secs: 15,
        }
    }
}

/// 存活对端：最近一次 resolve 决定 TTL 续期，过期由扫描判定下线。
struct LiveEntry {
    peer: PeerId,
    addrs: Vec<TransportAddr>,
    expires_at: Instant,
}

/// mDNS 发现源：以独立任务运行，事件推入 channel。
pub struct MdnsDiscovery {
    config: MdnsConfig,
}

impl MdnsDiscovery {
    pub fn new(config: MdnsConfig) -> Self {
        Self { config }
    }

    /// 本机通告信息：空地址 + addr_auto，由 mdns-sd 自动填充本机 IP。
    /// 输入均来自配置，格式非法（服务名/实例名/TXT 值）时返回 Err，由调用方发 Failed 事件。
    fn announce_info(&self) -> Result<ServiceInfo, String> {
        let port = self.config.quic_port.or(self.config.tcp_port).unwrap_or(0);
        let host = format!("{}.local.", self.config.instance);
        let props = encode_txt(
            &self.config.peer_id,
            self.config.quic_port,
            self.config.tcp_port,
        );
        ServiceInfo::new(
            &self.config.service_type,
            &self.config.instance,
            &host,
            (),
            port,
            props.as_slice(),
        )
        .map_err(|err| err.to_string())
        .map(ServiceInfo::enable_addr_auto)
    }

    /// 通告一次：构造或注册失败都发 Failed 事件（可观测，不 panic）。
    async fn announce_once(
        &self,
        daemon: &ServiceDaemon,
        events: &mpsc::Sender<DiscoveryEvent>,
        label: &str,
    ) {
        let info = match self.announce_info() {
            Ok(info) => info,
            Err(reason) => {
                emit_failed(events, format!("{label}: {reason}")).await;
                return;
            }
        };
        if let Err(err) = daemon.register(info) {
            emit_failed(events, format!("{label}: {err}")).await;
        }
    }

    /// 把 mdns-sd 事件映射为 DiscoveryEvent；跳过自身通告。
    /// 已知 peer 每次 resolved 都刷新 expires_at（mdns-sd 稳定期不重发 resolved，
    /// 由 run 周期性重启 browse 强制续期）；地址集变化才重发 Discovered。
    fn map_event(
        &self,
        live: &mut HashMap<String, LiveEntry>,
        ev: ServiceEvent,
        now: Instant,
    ) -> Option<DiscoveryEvent> {
        match ev {
            ServiceEvent::ServiceResolved(info) => {
                let (peer, addrs) = decode_txt(&info)?;
                if peer == self.config.peer_id {
                    return None;
                }
                let key = info.get_fullname().to_lowercase();
                let expires_at = now + Duration::from_secs(self.config.ttl_secs.into());
                let is_refresh = live.get(&key).is_some_and(|e| e.addrs == addrs);
                live.insert(
                    key,
                    LiveEntry {
                        peer,
                        addrs: addrs.clone(),
                        expires_at,
                    },
                );
                if is_refresh {
                    return None;
                }
                Some(DiscoveryEvent::Discovered(DiscoveredPeer {
                    peer,
                    addrs,
                    source: Source::Mdns,
                    expires_at: Some(expires_at),
                }))
            }
            ServiceEvent::ServiceRemoved(_, fullname) => live
                .remove(&fullname.to_lowercase())
                .map(|e| DiscoveryEvent::Expired(e.peer)),
            _ => None,
        }
    }

    /// 过期扫描：对超过 TTL 未续期的对端发射 Expired（恰好一次，随后移除）。
    fn expiry_scan(&self, live: &mut HashMap<String, LiveEntry>, now: Instant) -> Vec<PeerId> {
        let expired: Vec<(String, PeerId)> = live
            .iter()
            .filter(|(_, e)| e.expires_at <= now)
            .map(|(k, e)| (k.clone(), e.peer))
            .collect();
        let peers = expired.iter().map(|(_, p)| *p).collect();
        for (key, _) in expired {
            live.remove(&key);
        }
        peers
    }

    /// 启动一次浏览并把事件桥进共享 tokio 通道（多次浏览复用同一通道）。
    fn start_browse(
        &self,
        daemon: &ServiceDaemon,
        tx: mpsc::Sender<ServiceEvent>,
    ) -> Result<(), String> {
        match daemon.browse(&self.config.service_type) {
            Ok(flume_rx) => {
                bridge_browse(flume_rx, tx);
                Ok(())
            }
            Err(err) => Err(err.to_string()),
        }
    }

    /// 重启浏览（stop→browse）：启动期补错过的窗口，稳定期强制重新 resolve 续期。
    fn reprobe(&self, daemon: &ServiceDaemon, tx: &mpsc::Sender<ServiceEvent>) {
        if let Err(err) = daemon.stop_browse(&self.config.service_type) {
            tracing::warn!(target: "p2p_discovery", "mdns stop-browse: {err}");
        }
        if let Err(err) = self.start_browse(daemon, tx.clone()) {
            tracing::warn!(target: "p2p_discovery", "mdns re-browse: {err}");
        }
    }
}

/// 把 mdns-sd 的 flume 接收端桥接到 tokio channel（事件产生于其后台线程）。
fn bridge_browse(flume_rx: mdns_sd::Receiver<ServiceEvent>, tx: mpsc::Sender<ServiceEvent>) {
    std::thread::spawn(move || {
        for ev in flume_rx {
            if tx.blocking_send(ev).is_err() {
                break;
            }
        }
    });
}

/// 向事件通道发送 mDNS 失败事件（禁止静默吞错）。
async fn emit_failed(events: &mpsc::Sender<DiscoveryEvent>, reason: String) {
    let _ = events
        .send(DiscoveryEvent::Failed {
            source: Source::Mdns,
            reason,
        })
        .await;
}

#[async_trait::async_trait]
impl Discovery for MdnsDiscovery {
    fn name(&self) -> &'static str {
        "mdns"
    }

    async fn run(self: Arc<Self>, events: mpsc::Sender<DiscoveryEvent>) {
        let daemon = match ServiceDaemon::new() {
            Ok(d) => d,
            Err(err) => {
                emit_failed(&events, format!("daemon: {err}")).await;
                return;
            }
        };
        self.announce_once(&daemon, &events, "register").await;
        let (tx, mut rx) = mpsc::channel(64);
        if let Err(err) = self.start_browse(&daemon, tx.clone()) {
            emit_failed(&events, format!("browse: {err}")).await;
            return;
        }

        let mut live: HashMap<String, LiveEntry> = HashMap::new();
        let mut announce = tokio::time::interval_at(
            tokio::time::Instant::now() + self.config.announce_interval,
            self.config.announce_interval,
        );
        let mut scan =
            tokio::time::interval_at(tokio::time::Instant::now() + SCAN_INTERVAL, SCAN_INTERVAL);
        // 重询节奏（E4）：启动期短间隔补错过的公告/浏览窗口；稳定期每 TTL/2 重启浏览续期。
        let browse_refresh_secs = u64::from(self.config.ttl_secs / 2).max(1);
        let mut probe = probe::ProbeCadence::new(Duration::from_secs(browse_refresh_secs));
        let probe_sleep = tokio::time::sleep(probe.next());
        tokio::pin!(probe_sleep);

        'outer: loop {
            tokio::select! {
                maybe = rx.recv() => {
                    let Some(ev) = maybe else { break };
                    if let Some(discovery_ev) = self.map_event(&mut live, ev, Instant::now()) {
                        if events.send(discovery_ev).await.is_err() {
                            break;
                        }
                    }
                }
                _ = announce.tick() => {
                    // 周期重通告刷新 TTL（mdns-sd 允许对同一服务反复 register）
                    self.announce_once(&daemon, &events, "re-announce").await;
                }
                _ = scan.tick() => {
                    for peer in self.expiry_scan(&mut live, Instant::now()) {
                        if events.send(DiscoveryEvent::Expired(peer)).await.is_err() {
                            break 'outer;
                        }
                    }
                }
                _ = &mut probe_sleep => {
                    self.reprobe(&daemon, &tx);
                    probe_sleep.as_mut().reset(tokio::time::Instant::now() + probe.next());
                }
            }
        }
    }
}

mod probe;
mod txt;

#[cfg(test)]
mod tests;
