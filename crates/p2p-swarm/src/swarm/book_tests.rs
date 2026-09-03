//! 地址簿排序单测：mDNS > 同网段 > 全局 > 其余（E3），
//! 以及 hairpin 候选降权（E4：与自身观测地址同公网前缀的地址同级殿后）。

use p2p_identity::PeerId;
use p2p_transport::TransportAddr;

use super::book::{filter_loopback, AddrSource, AddressBook};

fn peer(n: u8) -> PeerId {
    PeerId::from_bytes([n; 32])
}

fn tcp(ip: &str, port: u16) -> TransportAddr {
    TransportAddr::Tcp {
        ip: ip.parse().unwrap(),
        port,
    }
}

/// 只取地址列（多数用例不关心 hairpin 标记）。
fn sorted_plain(book: &AddressBook, p: &PeerId) -> Vec<TransportAddr> {
    book.sorted_addrs(p, &[])
        .into_iter()
        .map(|(addr, _)| addr)
        .collect()
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
    let sorted = sorted_plain(&book, &p);
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
fn filter_loopback_keeps_all_loopback_set() {
    let addrs = vec![tcp("127.0.0.1", 1), tcp("127.0.0.1", 2)];
    let kept = filter_loopback(addrs.clone());
    assert_eq!(
        kept, addrs,
        "all-loopback set must be kept (same-host discovery)"
    );
}

#[test]
fn filter_loopback_drops_loopback_when_global_exists() {
    let addrs = vec![tcp("127.0.0.1", 1), tcp("240e:1000::5", 443)];
    let kept = filter_loopback(addrs);
    assert_eq!(kept, vec![tcp("240e:1000::5", 443)]);
}

#[test]
fn same_source_prefers_quic_before_tcp() {
    let mut book = AddressBook::new();
    let p = peer(3);
    let ip = "192.168.1.8";
    book.add(
        p,
        vec![
            (tcp(ip, 4001), AddrSource::Rendezvous),
            (
                TransportAddr::Quic {
                    ip: ip.parse().unwrap(),
                    port: 4000,
                },
                AddrSource::Rendezvous,
            ),
        ],
    );
    let sorted = sorted_plain(&book, &p);
    assert!(matches!(sorted[0], TransportAddr::Quic { port: 4000, .. }));
    assert!(matches!(sorted[1], TransportAddr::Tcp { port: 4001, .. }));
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
    let sorted = sorted_plain(&book, &p);
    assert_eq!(sorted[0], tcp("192.168.1.7", 8));
    assert_eq!(sorted[1], tcp("127.0.0.1", 9));
}

/// E4：与自身观测地址同公网前缀的登记地址是 hairpin 候选，
/// 必须在同级（rendezvous 全局）殿后，让位对端 LAN 地址。
#[test]
fn hairpin_candidate_demoted_after_lan_addr() {
    let mut book = AddressBook::new();
    let p = peer(4);
    let observed = vec![tcp("203.0.113.7", 45001)];
    // 登记顺序复刻残余缺陷：hairpin 候选（公网映射）登记在 LAN 地址之前
    book.add(
        p,
        vec![
            (tcp("203.0.113.7", 40000), AddrSource::Rendezvous),
            (tcp("192.168.1.10", 4000), AddrSource::Rendezvous),
        ],
    );
    let sorted = book.sorted_addrs(&p, &observed);
    assert_eq!(
        sorted,
        vec![
            (tcp("192.168.1.10", 4000), false),
            (tcp("203.0.113.7", 40000), true),
        ],
        "hairpin candidate must rank after the lan addr and be flagged"
    );
}

/// 无观测地址时 hairpin 判定不触发，排序与登记顺序一致（行为不回归）。
#[test]
fn without_observed_addrs_keeps_registration_order() {
    let mut book = AddressBook::new();
    let p = peer(5);
    book.add(
        p,
        vec![
            (tcp("203.0.113.7", 40000), AddrSource::Rendezvous),
            (tcp("192.168.1.10", 4000), AddrSource::Rendezvous),
        ],
    );
    let sorted = sorted_plain(&book, &p);
    assert_eq!(
        sorted,
        vec![tcp("203.0.113.7", 40000), tcp("192.168.1.10", 4000)],
        "no observed addrs must keep registration order"
    );
}

/// 私网观测地址（本机反射等单测输入）不得触发 hairpin 降权。
#[test]
fn private_observed_addr_does_not_demote() {
    let mut book = AddressBook::new();
    let p = peer(6);
    book.add(
        p,
        vec![
            (tcp("192.168.1.10", 4000), AddrSource::Rendezvous),
            (tcp("192.168.1.20", 5000), AddrSource::Rendezvous),
        ],
    );
    let sorted = book.sorted_addrs(&p, &[tcp("192.168.1.1", 1)]);
    assert!(
        sorted.iter().all(|(_, hairpin)| !hairpin),
        "private observed must not mark hairpin, got {sorted:?}"
    );
}

/// 聚合来源取最强档：Mdns > Rendezvous > Manual；手动登记不抹掉发现痕迹。
#[test]
fn aggregate_source_prefers_strongest_evidence() {
    let mut book = AddressBook::new();
    let p = peer(7);
    let (_, _, manual) = book.add(p, vec![(tcp("10.0.0.1", 1), AddrSource::Manual)]);
    assert_eq!(manual, AddrSource::Manual);
    let (_, _, rendezvous) = book.add(p, vec![(tcp("10.0.0.2", 2), AddrSource::Rendezvous)]);
    assert_eq!(rendezvous, AddrSource::Rendezvous);
    let (_, _, mdns) = book.add(p, vec![(tcp("192.168.1.30", 3), AddrSource::Mdns)]);
    assert_eq!(mdns, AddrSource::Mdns);
}
