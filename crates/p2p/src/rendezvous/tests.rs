//! rendezvous 接线单元测试（自 rendezvous.rs 拆出，为行数红线腾挪）。

use super::*;

/// 回归（E8-H3 panic 免除）：内置协议 ID 在装配期校验通过，且构造口可复用。
#[test]
fn builtin_rendezvous_id_valid_at_assembly() {
    assert!(RendezvousServer::with_public_only(false).is_ok());
    assert!(RendezvousServer::with_public_only(true).is_ok());
    assert_eq!(
        builtin_rendezvous_id().expect("valid builtin id").as_str(),
        RENDEZVOUS_PROTOCOL
    );
}

#[test]
fn parse_transport_addr_roundtrip() {
    let quic = parse_transport_addr("192.168.1.5/u40000").expect("quic addr");
    assert_eq!(
        quic,
        TransportAddr::Quic {
            ip: "192.168.1.5".parse().unwrap(),
            port: 40000
        }
    );
    let tcp = parse_transport_addr("127.0.0.1/t40001").expect("tcp addr");
    assert_eq!(
        tcp,
        TransportAddr::Tcp {
            ip: "127.0.0.1".parse().unwrap(),
            port: 40001
        }
    );
    assert!(parse_transport_addr("127.0.0.1/x1").is_err());
    assert!(parse_transport_addr("127.0.0.1/u").is_err());
    assert!(parse_transport_addr("no-slash/u1").is_err());
    assert!(parse_transport_addr("bad-ip/u1").is_err());
}
