//! 地址簿：PeerId → 带来源标记的地址列表，直连跳按优先级排序。
//!
//! 排序规则（design §7.2 + E3 同 NAT 缺陷）：
//! 1. mDNS 来源（同链路）最优先
//! 2. 与任一 mDNS 学习到的链路前缀同网段（v4 /24、v6 /64）次之
//! 3. rendezvous 观测到的全局地址再次
//! 4. 其余（显式登记、loopback 等）殿后
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
    /// 返回 (是否有新增, 该 peer 当前全部地址)。
    pub(crate) fn add(
        &mut self,
        peer: PeerId,
        addrs: Vec<(TransportAddr, AddrSource)>,
    ) -> (bool, Vec<TransportAddr>) {
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
        (added, all)
    }

    /// 直连跳用：按 优先级 + 登记顺序 排序后的地址列表。
    pub(crate) fn sorted_addrs(&self, peer: &PeerId) -> Vec<TransportAddr> {
        let Some(entry) = self.peers.get(peer) else {
            return Vec::new();
        };
        let mut ranked: Vec<(u8, usize, &BookedAddr)> = entry
            .iter()
            .enumerate()
            .map(|(idx, e)| (self.class_of(e), idx, e))
            .collect();
        ranked.sort_by_key(|(class, idx, _)| (*class, *idx));
        ranked.into_iter().map(|(_, _, e)| e.addr.clone()).collect()
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
        self.lan_ips.iter().any(|lan| match (lan, ip) {
            (IpAddr::V4(l), IpAddr::V4(i)) => l.octets()[..3] == i.octets()[..3],
            (IpAddr::V6(l), IpAddr::V6(i)) => l.octets()[..8] == i.octets()[..8],
            _ => false,
        })
    }
}

fn addr_ip(addr: &TransportAddr) -> IpAddr {
    match addr {
        TransportAddr::Quic { ip, .. } | TransportAddr::Tcp { ip, .. } => *ip,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(n: u8) -> PeerId {
        PeerId::from_bytes([n; 32])
    }

    fn tcp(ip: &str, port: u16) -> TransportAddr {
        TransportAddr::Tcp {
            ip: ip.parse().unwrap(),
            port,
        }
    }

    #[test]
    fn book_orders_mdns_lan_then_global() {
        let mut book = AddressBook::new();
        let p = peer(1);
        // 登记顺序：全局观测 → mDNS → 同网段观测（乱序注入，排序后应为 mdns→同网段→全局）
        book.add(
            p,
            vec![
                (tcp("10.99.99.99", 2), AddrSource::Rendezvous),
                (tcp("192.168.50.10", 1), AddrSource::Mdns),
                (tcp("192.168.50.20", 3), AddrSource::Rendezvous),
            ],
        );
        let sorted = book.sorted_addrs(&p);
        assert_eq!(
            sorted,
            vec![
                tcp("192.168.50.10", 1),
                tcp("192.168.50.20", 3),
                tcp("10.99.99.99", 2),
            ]
        );
    }

    #[test]
    fn manual_and_loopback_rank_last() {
        let mut book = AddressBook::new();
        let p = peer(2);
        book.add(
            p,
            vec![
                (tcp("127.0.0.1", 9), AddrSource::Manual),
                (tcp("192.168.1.7", 8), AddrSource::Rendezvous),
            ],
        );
        let sorted = book.sorted_addrs(&p);
        assert_eq!(sorted[0], tcp("192.168.1.7", 8));
        assert_eq!(sorted[1], tcp("127.0.0.1", 9));
    }
}
