//! 地址簿：PeerId → 带来源标记的地址列表，直连跳按优先级排序。
//!
//! 排序规则（design §7.2 + E3/E4 同 NAT 缺陷）：
//! 1. mDNS 来源（同链路）最优先
//! 2. 与任一 mDNS 学习到的链路前缀同网段（v4 /24、v6 /64）次之
//! 3. rendezvous 观测到的全局地址再次
//! 4. 其余（显式登记、loopback 等）殿后
//!
//! hairpin 降权（E4）：对端地址与自身观测地址同公网前缀（v4 /24、v6 /64）
//! 时大概率同 NAT，拨号走 NAT 内回环（hairpin）路径，多数 NAT 不支持或
//! 表现不稳定——该类地址在同级殿后，并由拨号侧施加短超时
//! （见 dial::HAIRPIN_DIAL_TIMEOUT），refused/黑洞不得吃满单地址预算。
//!
//! 同级保持登记顺序（stable sort）。

use std::collections::HashMap;
use std::net::IpAddr;

use p2p_identity::PeerId;
use p2p_transport::TransportAddr;

/// 地址来源（对齐 discovery 的 Source；Manual = 显式配置/静态登记）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddrSource {
    /// mDNS 同链路发现。
    Mdns,
    /// rendezvous 注册/查询学到的全局地址。
    Rendezvous,
    /// 显式配置或静态登记。
    Manual,
}

#[derive(Clone, PartialEq, Eq)]
struct BookedAddr {
    addr: TransportAddr,
    source: AddrSource,
}

/// 地址簿：同链路前缀（自 mDNS 地址学习）内的地址在直连跳排前。
pub(crate) struct AddressBook {
    peers: HashMap<PeerId, Vec<BookedAddr>>,
    /// mDNS 地址的 IP 集合（v4 按 /24、v6 按 /64 视为同链路前缀）。
    lan_ips: Vec<IpAddr>,
}

impl AddressBook {
    pub(crate) fn new() -> Self {
        Self {
            peers: HashMap::new(),
            lan_ips: Vec::new(),
        }
    }

    /// 登记地址（按地址去重，保留首见来源）；mDNS 来源学习链路前缀。
    /// 返回 (是否有新增, 该 peer 当前全部地址, 聚合展示来源)。
    pub(crate) fn add(
        &mut self,
        peer: PeerId,
        addrs: Vec<(TransportAddr, AddrSource)>,
    ) -> (bool, Vec<TransportAddr>, AddrSource) {
        // 入簿卫生（2026-09-04 线上串址实证）：链路本地地址缺接口作用域不可拨，
        // 一律不入簿；同一地址已被另一身份认领即串址污染，拒收并 WARN 留观测信号。
        let addrs: Vec<(TransportAddr, AddrSource)> = addrs
            .into_iter()
            .filter(|(addr, _)| {
                if is_link_local_ip(&addr_ip(addr)) {
                    tracing::debug!(%peer, %addr, "link-local address skipped: no scope, undialable");
                    return false;
                }
                if let Some(owner) = self.owner_of(addr) {
                    if owner != peer {
                        tracing::warn!(
                            %peer, owner = %owner, %addr,
                            "address claimed by another peer; dropped as suspected contamination"
                        );
                        return false;
                    }
                }
                true
            })
            .collect();
        let entry = self.peers.entry(peer).or_default();
        let before = entry.len();
        for (addr, source) in addrs {
            if entry.iter().any(|e| e.addr == addr) {
                continue;
            }
            if source == AddrSource::Mdns {
                let ip = addr_ip(&addr);
                if !self.lan_ips.contains(&ip) {
                    self.lan_ips.push(ip);
                }
            }
            entry.push(BookedAddr { addr, source });
        }
        let added = entry.len() > before;
        let all = entry.iter().map(|e| e.addr.clone()).collect();
        let source = aggregate_source(entry);
        (added, all, source)
    }

    /// 地址当前登记在哪个身份名下（串址检测）；未登记返回 None。
    fn owner_of(&self, addr: &TransportAddr) -> Option<PeerId> {
        self.peers
            .iter()
            .find(|(_, entry)| entry.iter().any(|e| e.addr == *addr))
            .map(|(p, _)| *p)
    }

    /// 直连跳用：按 (来源/网段优先级, hairpin 降权, 传输层, 登记顺序) 排序。
    /// 返回 (地址, 是否 hairpin 候选)；observed 为自身经地址观测学到的外部地址。
    pub(crate) fn sorted_addrs(
        &self,
        peer: &PeerId,
        observed: &[TransportAddr],
    ) -> Vec<(TransportAddr, bool)> {
        let Some(entry) = self.peers.get(peer) else {
            return Vec::new();
        };
        let mut ranked: Vec<(u8, bool, u8, usize, &BookedAddr)> = entry
            .iter()
            .enumerate()
            .map(|(idx, e)| {
                (
                    self.class_of(e),
                    is_hairpin_candidate(&e.addr, observed),
                    transport_rank(&e.addr),
                    idx,
                    e,
                )
            })
            .collect();
        ranked.sort_by_key(|(class, hairpin, transport, idx, _)| {
            (*class, *hairpin, *transport, *idx)
        });
        ranked
            .into_iter()
            .map(|(_, hairpin, _, _, e)| (e.addr.clone(), hairpin))
            .collect()
    }

    /// 优先级：mDNS=0 < 同链路网段=1 < rendezvous 全局=2 < 其余=3。
    fn class_of(&self, entry: &BookedAddr) -> u8 {
        match entry.source {
            AddrSource::Mdns => 0,
            _ => {
                if self.in_lan(&addr_ip(&entry.addr)) {
                    1
                } else if entry.source == AddrSource::Rendezvous {
                    2
                } else {
                    3
                }
            }
        }
    }

    /// 是否与任一学习到的链路前缀同网段。
    fn in_lan(&self, ip: &IpAddr) -> bool {
        self.lan_ips.iter().any(|lan| prefix_match(lan, ip))
    }
}

/// 同前缀判定：v4 /24、v6 /64（链路前缀与公网前缀共用口径）。
fn prefix_match(a: &IpAddr, b: &IpAddr) -> bool {
    match (a, b) {
        (IpAddr::V4(l), IpAddr::V4(i)) => l.octets()[..3] == i.octets()[..3],
        (IpAddr::V6(l), IpAddr::V6(i)) => l.octets()[..8] == i.octets()[..8],
        _ => false,
    }
}

/// hairpin 候选：对端全局地址与自身观测地址同公网前缀 → 大概率同 NAT。
/// 双方都要求全局段：观测源为 loopback/私网（如本机反射）不触发降权。
fn is_hairpin_candidate(addr: &TransportAddr, observed: &[TransportAddr]) -> bool {
    let ip = addr_ip(addr);
    if !is_global_ip(&ip) {
        return false;
    }
    observed
        .iter()
        .map(addr_ip)
        .any(|o| is_global_ip(&o) && prefix_match(&o, &ip))
}

/// 全局段判定：排除 loopback/私网/链路本地/未指定（v6 另排除 ULA fc00::/7）。
fn is_global_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            !v4.is_loopback() && !v4.is_private() && !v4.is_link_local() && !v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            !v6.is_loopback() && !v6.is_unspecified() && (v6.octets()[0] & 0xfe) != 0xfc
        }
    }
}

/// 过滤 loopback（127.0.0.0/8、::1）：仅当存在非 loopback 地址时移除——
/// loopback 对远端不可拨（E3：节点重启换端口后，对端拨其旧 loopback 注册项必拒）；
/// 全 loopback（同机部署/单测）保持原样以维持同机可发现性。
pub fn filter_loopback(addrs: Vec<TransportAddr>) -> Vec<TransportAddr> {
    let has_dialable = addrs.iter().any(|a| !is_loopback_addr(a));
    if has_dialable {
        addrs.into_iter().filter(|a| !is_loopback_addr(a)).collect()
    } else {
        addrs
    }
}

fn is_loopback_addr(addr: &TransportAddr) -> bool {
    match addr {
        TransportAddr::Quic { ip, .. } | TransportAddr::Tcp { ip, .. } => ip.is_loopback(),
    }
}

/// 同一来源/网段级别内优先 QUIC；其余排序仍由 class 主键决定。
fn transport_rank(addr: &TransportAddr) -> u8 {
    match addr {
        TransportAddr::Quic { .. } => 0,
        TransportAddr::Tcp { .. } => 1,
    }
}

/// 链路本地判定（入簿卫生）：v4 169.254/16、v6 fe80::/10——缺接口作用域不可拨。
fn is_link_local_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_unicast_link_local(),
    }
}

fn addr_ip(addr: &TransportAddr) -> IpAddr {
    match addr {
        TransportAddr::Quic { ip, .. } | TransportAddr::Tcp { ip, .. } => *ip,
    }
}

/// 对端聚合展示来源：Mdns > Rendezvous > Manual，覆盖面最强者优先。
/// 手动登记不抹掉发现痕迹：对已发现节点手动拨号不改变其来源标签。
fn aggregate_source(entry: &[BookedAddr]) -> AddrSource {
    let rank = |source: AddrSource| match source {
        AddrSource::Mdns => 2u8,
        AddrSource::Rendezvous => 1,
        AddrSource::Manual => 0,
    };
    let best = entry.iter().map(|e| rank(e.source)).max().unwrap_or(0);
    match best {
        2 => AddrSource::Mdns,
        1 => AddrSource::Rendezvous,
        _ => AddrSource::Manual,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(n: u8) -> PeerId {
        PeerId::from_bytes([n; 32])
    }

    fn quic(ip: IpAddr, port: u16) -> TransportAddr {
        TransportAddr::Quic { ip, port }
    }

    #[test]
    fn link_local_addresses_are_not_booked() {
        let mut book = AddressBook::new();
        let (added, all, _) = book.add(
            peer(1),
            vec![
                (quic("fe80::1".parse().unwrap(), 4000), AddrSource::Mdns),
                (quic("192.168.1.7".parse().unwrap(), 4000), AddrSource::Mdns),
            ],
        );
        assert!(added);
        assert_eq!(all, vec![quic("192.168.1.7".parse().unwrap(), 4000)]);
    }

    #[test]
    fn cross_peer_address_claim_is_dropped() {
        let mut book = AddressBook::new();
        let a = quic("192.168.1.7".parse().unwrap(), 4000);
        book.add(peer(1), vec![(a.clone(), AddrSource::Mdns)]);
        let (added, all, _) = book.add(peer(2), vec![(a, AddrSource::Rendezvous)]);
        assert!(!added, "contaminated claim must not enter the book");
        assert!(all.is_empty());
    }

    #[test]
    fn same_peer_duplicate_dedups_and_private_v4_survives() {
        let mut book = AddressBook::new();
        let a = quic("192.168.1.7".parse().unwrap(), 4000);
        book.add(peer(1), vec![(a.clone(), AddrSource::Mdns)]);
        let (added, all, _) = book.add(peer(1), vec![(a, AddrSource::Mdns)]);
        assert!(!added, "duplicate within same peer dedups");
        assert_eq!(all.len(), 1);
    }
}
