//! 契约 §2 事件联合 serde 单测：判别名/字段名逐字断言 + NodeEvent 全变体映射 + 可选 tsMs。

use super::testing::roundtrip;
use super::{HopKind, NodeEventJson, SourceKind};
use serde_json::{json, Value};

#[test]
fn node_event_discovered_connected_disconnected() {
    roundtrip(
        &NodeEventJson::PeerDiscovered {
            peer: "PeerA".into(),
            addrs: vec!["1.2.3.4/u3400".into()],
            source: SourceKind::Rendezvous,
            ts_ms: None,
        },
        json!({
            "type": "peer_discovered",
            "peer": "PeerA",
            "addrs": ["1.2.3.4/u3400"],
            "source": "rendezvous"
        }),
    );
    roundtrip(
        &NodeEventJson::PeerConnected {
            peer: "PeerA".into(),
            ts_ms: None,
        },
        json!({ "type": "peer_connected", "peer": "PeerA" }),
    );
    roundtrip(
        &NodeEventJson::PeerDisconnected {
            peer: "PeerA".into(),
            ts_ms: None,
        },
        json!({ "type": "peer_disconnected", "peer": "PeerA" }),
    );
}

#[test]
fn node_event_failure_variants() {
    roundtrip(
        &NodeEventJson::ListenFailed {
            addr: "0.0.0.0:3400".into(),
            reason: "占用".into(),
            ts_ms: None,
        },
        json!({ "type": "listen_failed", "addr": "0.0.0.0:3400", "reason": "占用" }),
    );
    roundtrip(
        &NodeEventJson::DialFailed {
            peer: None,
            reason: "不可达".into(),
            ts_ms: None,
        },
        json!({ "type": "dial_failed", "peer": null, "reason": "不可达" }),
    );
    roundtrip(
        &NodeEventJson::DialFailed {
            peer: Some("PeerB".into()),
            reason: "拒绝".into(),
            ts_ms: None,
        },
        json!({ "type": "dial_failed", "peer": "PeerB", "reason": "拒绝" }),
    );
    roundtrip(
        &NodeEventJson::ProtocolViolation {
            peer: "PeerC".into(),
            reason: "坏帧".into(),
            ts_ms: None,
        },
        json!({ "type": "protocol_violation", "peer": "PeerC", "reason": "坏帧" }),
    );
}

#[test]
fn node_event_dial_hop_relay_variant() {
    roundtrip(
        &NodeEventJson::DialHop {
            peer: "PeerD".into(),
            hop: HopKind::Relay,
            ok: false,
            detail: "relay down".into(),
            ts_ms: None,
        },
        json!({
            "type": "dial_hop",
            "peer": "PeerD",
            "hop": "relay",
            "ok": false,
            "detail": "relay down"
        }),
    );
}

#[test]
fn node_event_app_level_variants() {
    roundtrip(
        &NodeEventJson::NodeStarted {
            listen_addrs: vec!["127.0.0.1/u3400".into()],
            ts_ms: None,
        },
        json!({ "type": "node_started", "listenAddrs": ["127.0.0.1/u3400"] }),
    );
    roundtrip(
        &NodeEventJson::NodeStopped { ts_ms: None },
        json!({ "type": "node_stopped" }),
    );
    roundtrip(
        &NodeEventJson::NodeError {
            reason: "积压".into(),
            ts_ms: None,
        },
        json!({ "type": "node_error", "reason": "积压" }),
    );
}

#[test]
fn ts_ms_absent_when_none() {
    let encoded = serde_json::to_value(NodeEventJson::NodeStopped { ts_ms: None }).unwrap();
    assert!(encoded.get("tsMs").is_none(), "无值不得输出 tsMs 字段");
}

#[test]
fn ts_ms_stamped_on_every_variant() {
    let events = vec![
        NodeEventJson::PeerDiscovered {
            peer: "p".into(),
            addrs: vec![],
            source: SourceKind::Mdns,
            ts_ms: None,
        },
        NodeEventJson::PeerConnected {
            peer: "p".into(),
            ts_ms: None,
        },
        NodeEventJson::PeerDisconnected {
            peer: "p".into(),
            ts_ms: None,
        },
        NodeEventJson::ListenFailed {
            addr: "a".into(),
            reason: "r".into(),
            ts_ms: None,
        },
        NodeEventJson::DialFailed {
            peer: None,
            reason: "r".into(),
            ts_ms: None,
        },
        NodeEventJson::ProtocolViolation {
            peer: "p".into(),
            reason: "r".into(),
            ts_ms: None,
        },
        NodeEventJson::DialHop {
            peer: "p".into(),
            hop: HopKind::Direct,
            ok: true,
            detail: "d".into(),
            ts_ms: None,
        },
        NodeEventJson::NodeStarted {
            listen_addrs: vec![],
            ts_ms: None,
        },
        NodeEventJson::NodeStopped { ts_ms: None },
        NodeEventJson::NodeError {
            reason: "r".into(),
            ts_ms: None,
        },
    ];
    for event in events {
        let encoded = serde_json::to_value(event.clone().stamped(1700000000123)).unwrap();
        assert_eq!(
            encoded["tsMs"],
            Value::from(1_700_000_000_123u64),
            "变体 {} 缺 tsMs",
            encoded["type"]
        );
    }
}

#[test]
fn ts_ms_roundtrip_when_present() {
    let raw = json!({ "type": "node_error", "reason": "x", "tsMs": 42 });
    let decoded: NodeEventJson = serde_json::from_value(raw.clone()).expect("反序列化");
    match decoded.clone() {
        NodeEventJson::NodeError { reason, ts_ms } => {
            assert_eq!(reason, "x");
            assert_eq!(ts_ms, Some(42));
        }
        other => panic!("期望 node_error，实得 {other:?}"),
    }
    roundtrip(&decoded, raw);
}

#[test]
fn node_event_from_kernel_event_maps_every_variant() {
    let peer = p2p::PeerId::from_bytes([7u8; 32]);
    let peer_str = peer.to_string();
    let cases: Vec<(p2p::NodeEvent, NodeEventJson)> = vec![
        (
            p2p::NodeEvent::PeerDiscovered {
                peer,
                addrs: vec!["1.2.3.4/u3400".into()],
                source: p2p_swarm::AddrSource::Rendezvous,
            },
            NodeEventJson::PeerDiscovered {
                peer: peer_str.clone(),
                addrs: vec!["1.2.3.4/u3400".into()],
                source: SourceKind::Rendezvous,
                ts_ms: None,
            },
        ),
        (
            p2p::NodeEvent::PeerConnected { peer },
            NodeEventJson::PeerConnected {
                peer: peer_str.clone(),
                ts_ms: None,
            },
        ),
        (
            p2p::NodeEvent::PeerDisconnected { peer },
            NodeEventJson::PeerDisconnected {
                peer: peer_str.clone(),
                ts_ms: None,
            },
        ),
        (
            p2p::NodeEvent::ListenFailed {
                addr: "0.0.0.0:1".into(),
                reason: "x".into(),
            },
            NodeEventJson::ListenFailed {
                addr: "0.0.0.0:1".into(),
                reason: "x".into(),
                ts_ms: None,
            },
        ),
        (
            p2p::NodeEvent::DialFailed {
                peer: None,
                reason: "x".into(),
            },
            NodeEventJson::DialFailed {
                peer: None,
                reason: "x".into(),
                ts_ms: None,
            },
        ),
        (
            p2p::NodeEvent::ProtocolViolation {
                peer,
                reason: "x".into(),
            },
            NodeEventJson::ProtocolViolation {
                peer: peer_str.clone(),
                reason: "x".into(),
                ts_ms: None,
            },
        ),
        (
            p2p::NodeEvent::DialHop {
                peer,
                hop: p2p_swarm::DialHop::Punch,
                ok: true,
                detail: "ok".into(),
            },
            NodeEventJson::DialHop {
                peer: peer_str,
                hop: HopKind::Punch,
                ok: true,
                detail: "ok".into(),
                ts_ms: None,
            },
        ),
    ];
    for (kernel, expected) in cases {
        assert_eq!(NodeEventJson::from(kernel), expected);
    }
}

#[test]
fn hop_kind_maps_all_kernel_variants() {
    assert_eq!(HopKind::from(p2p_swarm::DialHop::Direct), HopKind::Direct);
    assert_eq!(HopKind::from(p2p_swarm::DialHop::Punch), HopKind::Punch);
    assert_eq!(HopKind::from(p2p_swarm::DialHop::Relay), HopKind::Relay);
}
