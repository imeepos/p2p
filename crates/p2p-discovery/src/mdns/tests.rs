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
    live.insert(fullname.clone(), peer);
    let ev = disc.map_event(
        &mut live,
        ServiceEvent::ServiceRemoved(SERVICE_TYPE.to_string(), fullname),
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
    );
    assert!(ev.is_none());
    assert!(live.is_empty());
}
