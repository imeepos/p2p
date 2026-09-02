//! mDNS 局域网发现（design §7.1）：通告本机 + 浏览局域网，事件推入统一 channel。

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use p2p_identity::PeerId;
use p2p_transport::TransportAddr;
use tokio::sync::mpsc;

use crate::{DiscoveredPeer, Discovery, DiscoveryEvent, Source};

/// 服务类型：局域网上所有 p2p-base 节点共用。
pub const SERVICE_TYPE: &str = "_p2pbase._udp.local";

const TXT_KEY_PEER: &str = "peer";
const TXT_KEY_QUIC: &str = "quic";
const TXT_KEY_TCP: &str = "tcp";

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

/// 编码通告 TXT 记录：peer(base58) + 可选 quic/tcp 端口。
fn encode_txt(peer: &PeerId, quic: Option<u16>, tcp: Option<u16>) -> Vec<(&'static str, String)> {
    let mut props = vec![(TXT_KEY_PEER, peer.to_string())];
    if let Some(port) = quic {
        props.push((TXT_KEY_QUIC, port.to_string()));
    }
    if let Some(port) = tcp {
        props.push((TXT_KEY_TCP, port.to_string()));
    }
    props
}

/// 从已解析的 ServiceInfo 解码 (PeerId, 地址列表)。peer 缺失或端口非法返回 None。
fn decode_txt(info: &ServiceInfo) -> Option<(PeerId, Vec<TransportAddr>)> {
    let peer_b58 = info.get_property_val_str(TXT_KEY_PEER)?;
    let bytes: [u8; 32] = bs58::decode(peer_b58).into_vec().ok()?.try_into().ok()?;
    let peer = PeerId::from_bytes(bytes);
    let ip: IpAddr = *info.get_addresses().iter().next()?;
    let mut addrs = Vec::new();
    if let Some(port) = txt_port(info, TXT_KEY_QUIC) {
        addrs.push(TransportAddr::Quic { ip, port });
    }
    if let Some(port) = txt_port(info, TXT_KEY_TCP) {
        addrs.push(TransportAddr::Tcp { ip, port });
    }
    (!addrs.is_empty()).then_some((peer, addrs))
}

/// 读取 TXT 端口属性并解析为 u16。
fn txt_port(info: &ServiceInfo, key: &str) -> Option<u16> {
    info.get_property_val_str(key).and_then(|s| s.parse().ok())
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
    fn announce_info(&self) -> ServiceInfo {
        let port = self.config.quic_port.or(self.config.tcp_port).unwrap_or(0);
        let host = format!("{}.local", self.config.instance);
        let props = encode_txt(&self.config.peer_id, self.config.quic_port, self.config.tcp_port);
        ServiceInfo::new(
            &self.config.service_type,
            &self.config.instance,
            &host,
            (),
            port,
            props.as_slice(),
        )
        .expect("valid mdns service info")
        .enable_addr_auto()
    }

    /// 把 mdns-sd 事件映射为 DiscoveryEvent；跳过自身通告。
    fn map_event(&self, live: &mut HashMap<String, PeerId>, ev: ServiceEvent) -> Option<DiscoveryEvent> {
        match ev {
            ServiceEvent::ServiceResolved(info) => {
                let (peer, addrs) = decode_txt(&info)?;
                if peer == self.config.peer_id {
                    return None;
                }
                live.insert(info.get_fullname().to_lowercase(), peer);
                Some(DiscoveryEvent::Discovered(DiscoveredPeer {
                    peer,
                    addrs,
                    source: Source::Mdns,
                    expires_at: Some(Instant::now() + Duration::from_secs(self.config.ttl_secs.into())),
                }))
            }
            ServiceEvent::ServiceRemoved(_, fullname) => live.remove(&fullname.to_lowercase()).map(DiscoveryEvent::Expired),
            _ => None,
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
        .send(DiscoveryEvent::Failed { source: Source::Mdns, reason })
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
        if let Err(err) = daemon.register(self.announce_info()) {
            emit_failed(&events, format!("register: {err}")).await;
        }
        let (tx, mut rx) = mpsc::channel(64);
        match daemon.browse(&self.config.service_type) {
            Ok(flume_rx) => bridge_browse(flume_rx, tx),
            Err(err) => {
                emit_failed(&events, format!("browse: {err}")).await;
                return;
            }
        }
        let mut live: HashMap<String, PeerId> = HashMap::new();
        let mut announce = tokio::time::interval_at(
            tokio::time::Instant::now() + self.config.announce_interval,
            self.config.announce_interval,
        );
        loop {
            tokio::select! {
                maybe = rx.recv() => {
                    let Some(ev) = maybe else { break };
                    if let Some(discovery_ev) = self.map_event(&mut live, ev) {
                        if events.send(discovery_ev).await.is_err() {
                            break;
                        }
                    }
                }
                _ = announce.tick() => {
                    // 周期重通告刷新 TTL（mdns-sd 允许对同一服务反复 register）
                    if let Err(err) = daemon.register(self.announce_info()) {
                        emit_failed(&events, format!("re-announce: {err}")).await;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_info(peer: PeerId, quic: u16, tcp: u16) -> ServiceInfo {
        let props = encode_txt(&peer, Some(quic), Some(tcp));
        ServiceInfo::new(
            SERVICE_TYPE,
            "node-test",
            "node-test.local",
            IpAddr::from([192, 168, 1, 50]),
            quic,
            props.as_slice(),
        )
        .expect("valid service info")
    }

    #[test]
    fn txt_roundtrip() {
        let peer = PeerId::from_bytes([7u8; 32]);
        let info = sample_info(peer, 12345, 12346);
        let (decoded, addrs) = decode_txt(&info).expect("decode");
        assert_eq!(decoded, peer);
        assert_eq!(addrs.len(), 2);
        let quic = TransportAddr::Quic { ip: IpAddr::from([192, 168, 1, 50]), port: 12345 };
        let tcp = TransportAddr::Tcp { ip: IpAddr::from([192, 168, 1, 50]), port: 12346 };
        assert!(addrs.contains(&quic));
        assert!(addrs.contains(&tcp));
    }

    #[test]
    fn txt_decode_rejects_garbage_peer() {
        let props = [("peer", "!!!not-base58!!!".to_string())];
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            "node-bad",
            "node-bad.local",
            IpAddr::from([192, 168, 1, 51]),
            8000,
            props.as_slice(),
        )
        .expect("valid service info");
        assert!(decode_txt(&info).is_none());
    }

    #[test]
    fn map_resolved_emits_discovered() {
        let peer = PeerId::from_bytes([8u8; 32]);
        let disc = MdnsDiscovery::new(MdnsConfig::new(PeerId::from_bytes([9u8; 32])));
        let mut live = HashMap::new();
        let ev = disc.map_event(&mut live, ServiceEvent::ServiceResolved(sample_info(peer, 4000, 4001)));
        match ev {
            Some(DiscoveryEvent::Discovered(dp)) => {
                assert_eq!(dp.peer, peer);
                assert_eq!(dp.source, Source::Mdns);
                assert!(dp.expires_at.is_some());
                assert_eq!(dp.addrs.len(), 2);
            }
            other => panic!("expected Discovered, got {other:?}"),
        }
        assert_eq!(live.len(), 1);
    }

    #[test]
    fn map_removed_emits_expired() {
        let peer = PeerId::from_bytes([10u8; 32]);
        let disc = MdnsDiscovery::new(MdnsConfig::new(PeerId::from_bytes([9u8; 32])));
        let fullname = sample_info(peer, 4000, 4001).get_fullname().to_lowercase();
        let mut live = HashMap::new();
        live.insert(fullname.clone(), peer);
        let ev = disc.map_event(&mut live, ServiceEvent::ServiceRemoved(SERVICE_TYPE.to_string(), fullname));
        assert!(matches!(ev, Some(DiscoveryEvent::Expired(p)) if p == peer));
        assert!(live.is_empty());
    }

    #[test]
    fn map_skips_own_announcement() {
        let peer = PeerId::from_bytes([11u8; 32]);
        let disc = MdnsDiscovery::new(MdnsConfig::new(peer));
        let mut live = HashMap::new();
        let ev = disc.map_event(&mut live, ServiceEvent::ServiceResolved(sample_info(peer, 5000, 5001)));
        assert!(ev.is_none());
        assert!(live.is_empty());
    }
}