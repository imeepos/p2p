use super::*;

#[test]
fn service_type_ends_with_valid_mdns_domain() {
    // mdns-sd 0.13 register/browse 校验要求类型以 '._tcp.local.' 或 '._udp.local.'（带尾点）结尾；
    // 缺尾点会导致 E1 实测 WARN register/browse 报错、mDNS 完全不工作。
    assert!(
        SERVICE_TYPE.ends_with("._udp.local."),
        "SERVICE_TYPE 必须以带尾点的 ._udp.local. 结尾，当前: {SERVICE_TYPE}"
    );
    // 通告与浏览共用同一类型，构造 ServiceInfo 应成功
    let props = encode_txt(&PeerId::from_bytes([1u8; 32]), None, None);
    let info = ServiceInfo::new(
        SERVICE_TYPE,
        "node-type-check",
        "node-type-check.local",
        IpAddr::from([192, 168, 1, 60]),
        1234,
        props.as_slice(),
    )
    .expect("valid service info");
    assert_eq!(info.get_type(), SERVICE_TYPE);
}

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
    let quic = TransportAddr::Quic {
        ip: IpAddr::from([192, 168, 1, 50]),
        port: 12345,
    };
    let tcp = TransportAddr::Tcp {
        ip: IpAddr::from([192, 168, 1, 50]),
        port: 12346,
    };
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
    let ev = disc.map_event(
        &mut live,
        ServiceEvent::ServiceResolved(sample_info(peer, 4000, 4001)),
        Instant::now(),
    );
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
    live.insert(
        fullname.clone(),
        LiveEntry {
            peer,
            addrs: Vec::new(),
            expires_at: Instant::now(),
        },
    );
    let ev = disc.map_event(
        &mut live,
        ServiceEvent::ServiceRemoved(SERVICE_TYPE.to_string(), fullname),
        Instant::now(),
    );
    assert!(matches!(ev, Some(DiscoveryEvent::Expired(p)) if p == peer));
    assert!(live.is_empty());
}

#[test]
fn map_skips_own_announcement() {
    let peer = PeerId::from_bytes([11u8; 32]);
    let disc = MdnsDiscovery::new(MdnsConfig::new(peer));
    let mut live = HashMap::new();
    let ev = disc.map_event(
        &mut live,
        ServiceEvent::ServiceResolved(sample_info(peer, 5000, 5001)),
        Instant::now(),
    );
    assert!(ev.is_none());
    assert!(live.is_empty());
}

#[test]
fn announce_hostname_ends_with_local_dot() {
    // mdns-sd 运行期校验 hostname 必须以 '.local.' 结尾；缺尾点会让
    // register/re-announce 全部失败（E1 实测：p2p-XXXXXXXX.local 被拒）。
    let disc = MdnsDiscovery::new(MdnsConfig::new(PeerId::from_bytes([12u8; 32])));
    let info = disc.announce_info();
    let host = info.get_hostname();
    assert!(
        host.ends_with(".local."),
        "hostname 必须以 .local. 结尾，当前: {host}"
    );
}
#[test]
fn silent_peer_expires_exactly_once_after_ttl() {
    // 回归（E1 假阳性）：resolved 一次后静默 >TTL → 过期扫描恰好发一次 Expired
    let peer = PeerId::from_bytes([20u8; 32]);
    let disc = MdnsDiscovery::new(MdnsConfig::new(PeerId::from_bytes([21u8; 32])));
    let mut live = HashMap::new();
    let t0 = Instant::now();
    let ev = disc.map_event(
        &mut live,
        ServiceEvent::ServiceResolved(sample_info(peer, 7000, 7001)),
        t0,
    );
    assert!(matches!(ev, Some(DiscoveryEvent::Discovered(_))));
    assert_eq!(live.len(), 1);

    // TTL 未到：不扫出
    let before = disc.expiry_scan(
        &mut live,
        t0 + Duration::from_secs(disc.config.ttl_secs.into()) - Duration::from_millis(1),
    );
    assert!(before.is_empty());

    // TTL 已到且无续期：恰好一次
    let expired = disc.expiry_scan(
        &mut live,
        t0 + Duration::from_secs(disc.config.ttl_secs.into()),
    );
    assert_eq!(expired, vec![peer]);
    assert!(live.is_empty());

    // 再扫：不重复
    let again = disc.expiry_scan(&mut live, t0 + Duration::from_secs(1000));
    assert!(again.is_empty());
}

#[test]
fn refreshed_peer_never_expires() {
    // 回归（E1 漏报的反面）：持续 resolved 续期 → 永不 Expired
    let peer = PeerId::from_bytes([30u8; 32]);
    let disc = MdnsDiscovery::new(MdnsConfig::new(PeerId::from_bytes([31u8; 32])));
    let mut live = HashMap::new();
    let ttl = Duration::from_secs(disc.config.ttl_secs.into());
    let mut t = Instant::now();
    let ev = disc.map_event(
        &mut live,
        ServiceEvent::ServiceResolved(sample_info(peer, 8000, 8001)),
        t,
    );
    assert!(matches!(ev, Some(DiscoveryEvent::Discovered(_))));
    // 每个 TTL/2 持续 resolved（模拟浏览重启续期），绝不过期
    for _ in 0..5 {
        t += ttl / 2;
        // 在上一次续期窗口内（尚未到 expires_at）不应过期
        assert!(
            disc.expiry_scan(&mut live, t).is_empty(),
            "续期窗口内不应过期"
        );
        let ev = disc.map_event(
            &mut live,
            ServiceEvent::ServiceResolved(sample_info(peer, 8000, 8001)),
            t,
        );
        assert!(ev.is_none(), "稳定续期不应重复发 Discovered");
    }
    // 最后一次续期后，expires_at = t + ttl；到达前一刻仍存活
    assert!(disc
        .expiry_scan(&mut live, t + ttl - Duration::from_millis(1))
        .is_empty());
    assert_eq!(live.len(), 1);
}

#[test]
fn changed_addrs_reemit_discovered_but_same_addrs_just_refresh() {
    // 地址集变化才重发 Discovered；同地址只续期
    let peer = PeerId::from_bytes([40u8; 32]);
    let disc = MdnsDiscovery::new(MdnsConfig::new(PeerId::from_bytes([41u8; 32])));
    let mut live = HashMap::new();
    let t0 = Instant::now();
    assert!(matches!(
        disc.map_event(
            &mut live,
            ServiceEvent::ServiceResolved(sample_info(peer, 9000, 9001)),
            t0,
        ),
        Some(DiscoveryEvent::Discovered(_))
    ));
    // 同地址第二次 resolved：只续期不重发
    assert!(disc
        .map_event(
            &mut live,
            ServiceEvent::ServiceResolved(sample_info(peer, 9000, 9001)),
            t0,
        )
        .is_none());
    // 地址变化：重发 Discovered
    let ev = disc.map_event(
        &mut live,
        ServiceEvent::ServiceResolved(sample_info(peer, 9100, 9101)),
        t0,
    );
    match ev {
        Some(DiscoveryEvent::Discovered(dp)) => assert_eq!(dp.addrs.len(), 2),
        other => panic!("地址变化应重发 Discovered, got {other:?}"),
    }
}
