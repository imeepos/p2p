//! 应答快照缓存回归：重复查询削峰编码、register 失效、条目真实 TTL 重建。

use super::*;
use p2p_identity::Keypair;

use crate::rendezvous::messages::{sign_register, unix_now, Query, Response};

fn sample_addrs() -> Vec<TransportAddr> {
    vec![TransportAddr::Quic {
        ip: "10.0.0.5".parse().unwrap(),
        port: 4000,
    }]
}

fn valid_reg(namespace: &str, ttl: u32) -> Register {
    let kp = Keypair::generate();
    sign_register(&kp, namespace, &sample_addrs(), ttl, unix_now())
}

fn full_query(namespace: &str) -> Query {
    Query {
        namespace: namespace.to_string(),
        peer_id: Vec::new(),
    }
}

#[test]
fn snapshot_cache_serves_repeats_and_rebuilds_on_register() {
    let registry = RendezvousRegistry::new();
    registry
        .register(&valid_reg("room-a", 60), unix_now())
        .expect("register ok");
    let q = full_query("room-a");

    let first = registry.query_encoded(&q);
    assert_eq!(registry.snapshot_rebuild_count(), 1);
    let second = registry.query_encoded(&q);
    assert_eq!(first, second, "两次全量查询应答必须逐字节一致");
    assert_eq!(
        registry.snapshot_rebuild_count(),
        1,
        "第二次全量查询必须命中快照缓存，不得重建"
    );

    // 新 peer 入册改变内容：缓存失效并重建，应答含两条
    registry
        .register(&valid_reg("room-a", 60), unix_now())
        .expect("register 2 ok");
    let third = registry.query_encoded(&q);
    assert_eq!(registry.snapshot_rebuild_count(), 2);
    let resp = Response::decode(third.as_slice()).expect("decode ok");
    assert_eq!(resp.peers.len(), 2, "重建后的快照必须包含新注册的对端");
}

#[tokio::test]
async fn snapshot_cache_never_serves_expired_entries() {
    let registry = RendezvousRegistry::new();
    registry
        .register(&valid_reg("room-b", 1), unix_now())
        .expect("register ok");
    let q = full_query("room-b");
    let _ = registry.query_encoded(&q);
    assert_eq!(registry.snapshot_rebuild_count(), 1);

    tokio::time::sleep(Duration::from_millis(1100)).await;
    let second = registry.query_encoded(&q);
    assert_eq!(
        registry.snapshot_rebuild_count(),
        2,
        "条目真实 TTL 到期必须重建，不得回放陈旧快照"
    );
    let resp = Response::decode(second.as_slice()).expect("decode ok");
    assert!(resp.peers.is_empty(), "过期条目不得出现在快照应答中");
}

#[test]
fn targeted_query_reads_key_without_full_snapshot() {
    // P1 查号台语义：单 peer 查询按键读取，未注册对端返回空应答
    let registry = RendezvousRegistry::new();
    let reg = valid_reg("room-d", 60);
    let peer_bytes = reg.peer_id.clone();
    registry.register(&reg, unix_now()).expect("register ok");
    let unknown = Query {
        namespace: "room-d".to_string(),
        peer_id: [9u8; 32].to_vec(),
    };
    let resp = Response::decode(registry.query_encoded(&unknown).as_slice()).expect("decode ok");
    assert!(resp.peers.is_empty(), "未注册对端必须空应答");
    let known = Query {
        namespace: "room-d".to_string(),
        peer_id: peer_bytes,
    };
    let resp = Response::decode(registry.query_encoded(&known).as_slice()).expect("decode ok");
    assert_eq!(resp.peers.len(), 1, "键读取必须命中已注册对端");
}

#[test]
fn single_peer_query_bypasses_snapshot_cache() {
    let registry = RendezvousRegistry::new();
    let reg = valid_reg("room-c", 60);
    let peer_bytes = reg.peer_id.clone();
    registry.register(&reg, unix_now()).expect("register ok");
    let q = Query {
        namespace: "room-c".to_string(),
        peer_id: peer_bytes,
    };
    let first = registry.query_encoded(&q);
    let second = registry.query_encoded(&q);
    assert_eq!(
        registry.snapshot_rebuild_count(),
        0,
        "单 peer 精确查询不进快照缓存"
    );
    assert_eq!(first, second);
    let resp = Response::decode(first.as_slice()).expect("decode ok");
    assert_eq!(resp.peers.len(), 1);
}
