//! 契约 serde 单测（gui-contract.md §2/§3）：camelCase 字段逐字断言 + 全类型 roundtrip。

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use super::*;

/// 编码结果必须与契约 JSON 逐字段一致，且能从契约 JSON 原样还原。
fn roundtrip<T>(value: T, raw: Value)
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let encoded = serde_json::to_value(&value).expect("序列化");
    assert_eq!(encoded, raw, "序列化字段与契约不一致");
    let decoded: T = serde_json::from_value(raw).expect("反序列化");
    assert_eq!(decoded, value, "roundtrip 不保真");
}

fn sample_config() -> GuiConfig {
    GuiConfig {
        quic_port: 3400,
        tcp_port: 3401,
        enable_mdns: true,
        data_dir: "/data/p2p-data".into(),
        bootstrap: vec!["1.2.3.4/3400".into(), "1.2.3.4/t3401".into()],
        relay_addrs: vec!["5.6.7.8/3400".into()],
        advertised_addrs: vec!["9.9.9.9/4000".into()],
        observation_port: Some(3402),
        observation_addrs: vec!["1.2.3.4:3402".into()],
    }
}

#[test]
fn gui_config_camel_case_roundtrip() {
    roundtrip(
        sample_config(),
        json!({
            "quicPort": 3400,
            "tcpPort": 3401,
            "enableMdns": true,
            "dataDir": "/data/p2p-data",
            "bootstrap": ["1.2.3.4/3400", "1.2.3.4/t3401"],
            "relayAddrs": ["5.6.7.8/3400"],
            "advertisedAddrs": ["9.9.9.9/4000"],
            "observationPort": 3402,
            "observationAddrs": ["1.2.3.4:3402"],
        }),
    );
}

#[test]
fn gui_config_optional_port_null_roundtrip() {
    roundtrip(
        GuiConfig {
            observation_port: None,
            ..sample_config()
        },
        json!({
            "quicPort": 3400,
            "tcpPort": 3401,
            "enableMdns": true,
            "dataDir": "/data/p2p-data",
            "bootstrap": ["1.2.3.4/3400", "1.2.3.4/t3401"],
            "relayAddrs": ["5.6.7.8/3400"],
            "advertisedAddrs": ["9.9.9.9/4000"],
            "observationPort": null,
            "observationAddrs": ["1.2.3.4:3402"],
        }),
    );
}

#[test]
fn node_status_running_shape() {
    let status = NodeStatus {
        running: true,
        peer_id: Some("3xY9abc".into()),
        listen_addrs: vec!["127.0.0.1/3400".into()],
        uptime_secs: 42,
        started_at_ms: Some(1_700_000_000_000),
        config: sample_config(),
    };
    roundtrip(
        status,
        json!({
            "running": true,
            "peerId": "3xY9abc",
            "listenAddrs": ["127.0.0.1/3400"],
            "uptimeSecs": 42,
            "startedAtMs": 1_700_000_000_000u64,
            "config": serde_json::to_value(sample_config()).unwrap(),
        }),
    );
}

#[test]
fn node_status_stopped_option_fields_are_null() {
    let status = NodeStatus {
        running: false,
        peer_id: None,
        listen_addrs: Vec::new(),
        uptime_secs: 0,
        started_at_ms: None,
        config: sample_config(),
    };
    let encoded = serde_json::to_value(&status).unwrap();
    assert_eq!(encoded["peerId"], Value::Null);
    assert_eq!(encoded["startedAtMs"], Value::Null);
    let decoded: NodeStatus = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, status);
}

#[test]
fn metrics_json_all_fields_camel_case() {
    let metrics = MetricsJson {
        dial_direct_ok: 1,
        dial_direct_fail: 2,
        dial_punch_ok: 3,
        dial_punch_fail: 4,
        dial_relay_ok: 5,
        dial_relay_fail: 6,
        addr_dial_failures: 7,
        relay_reconnects: 8,
        gate_denials_total: 9,
        active_connections: 10,
        relay_sessions_active: 11,
    };
    roundtrip(
        metrics,
        json!({
            "dialDirectOk": 1, "dialDirectFail": 2,
            "dialPunchOk": 3, "dialPunchFail": 4,
            "dialRelayOk": 5, "dialRelayFail": 6,
            "addrDialFailures": 7,
            "relayReconnects": 8,
            "gateDenialsTotal": 9,
            "activeConnections": 10,
            "relaySessionsActive": 11,
        }),
    );
}

#[test]
fn dial_report_and_hop_shapes() {
    let report = DialReport {
        peer: "3xY9abc".into(),
        hops: vec![
            DialHopJson {
                hop: HopKind::Direct,
                ok: false,
                detail: "no route".into(),
            },
            DialHopJson {
                hop: HopKind::Punch,
                ok: true,
                detail: "punched".into(),
            },
        ],
        ok: true,
        total_ms: 88,
    };
    roundtrip(
        report,
        json!({
            "peer": "3xY9abc",
            "hops": [
                { "hop": "direct", "ok": false, "detail": "no route" },
                { "hop": "punch", "ok": true, "detail": "punched" }
            ],
            "ok": true,
            "totalMs": 88,
        }),
    );
}

#[test]
fn ping_outcome_success_and_failure_shapes() {
    let ok = PingOutcome {
        ok: true,
        rtt_ms: Some(12),
        hops: Vec::new(),
        error: None,
    };
    roundtrip(
        ok.clone(),
        json!({ "ok": true, "rttMs": 12, "hops": [], "error": null }),
    );
    let failed = PingOutcome {
        ok: false,
        rtt_ms: None,
        hops: Vec::new(),
        error: Some("超时".into()),
    };
    roundtrip(
        failed,
        json!({ "ok": false, "rttMs": null, "hops": [], "error": "超时" }),
    );
}

#[test]
fn node_event_discovered_connected_disconnected() {
    roundtrip(
        NodeEventJson::PeerDiscovered {
            peer: "PeerA".into(),
            addrs: vec!["1.2.3.4/3400".into()],
        },
        json!({ "type": "peer_discovered", "peer": "PeerA", "addrs": ["1.2.3.4/3400"] }),
    );
    roundtrip(
        NodeEventJson::PeerConnected { peer: "PeerA".into() },
        json!({ "type": "peer_connected", "peer": "PeerA" }),
    );
    roundtrip(
        NodeEventJson::PeerDisconnected { peer: "PeerA".into() },
        json!({ "type": "peer_disconnected", "peer": "PeerA" }),
    );
}

#[test]
fn node_event_failure_variants() {
    roundtrip(
        NodeEventJson::ListenFailed {
            addr: "0.0.0.0:3400".into(),
            reason: "占用".into(),
        },
        json!({ "type": "listen_failed", "addr": "0.0.0.0:3400", "reason": "占用" }),
    );
    roundtrip(
        NodeEventJson::DialFailed {
            peer: None,
            reason: "不可达".into(),
        },
        json!({ "type": "dial_failed", "peer": null, "reason": "不可达" }),
    );
    roundtrip(
        NodeEventJson::DialFailed {
            peer: Some("PeerB".into()),
            reason: "拒绝".into(),
        },
        json!({ "type": "dial_failed", "peer": "PeerB", "reason": "拒绝" }),
    );
    roundtrip(
        NodeEventJson::ProtocolViolation {
            peer: "PeerC".into(),
            reason: "坏帧".into(),
        },
        json!({ "type": "protocol_violation", "peer": "PeerC", "reason": "坏帧" }),
    );
}

#[test]
fn node_event_dial_hop_relay_variant() {
    roundtrip(
        NodeEventJson::DialHop {
            peer: "PeerD".into(),
            hop: HopKind::Relay,
            ok: false,
            detail: "relay down".into(),
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
        NodeEventJson::NodeStarted {
            listen_addrs: vec!["127.0.0.1/3400".into()],
        },
        json!({ "type": "node_started", "listenAddrs": ["127.0.0.1/3400"] }),
    );
    roundtrip(NodeEventJson::NodeStopped, json!({ "type": "node_stopped" }));
    roundtrip(
        NodeEventJson::NodeError {
            reason: "积压".into(),
        },
        json!({ "type": "node_error", "reason": "积压" }),
    );
}

#[test]
fn node_event_from_kernel_event_maps_every_variant() {
    let peer = p2p::PeerId::from_bytes([7u8; 32]);
    let peer_str = peer.to_string();
    let cases: Vec<(p2p::NodeEvent, NodeEventJson)> = vec![
        (
            p2p::NodeEvent::PeerDiscovered {
                peer,
                addrs: vec!["1.2.3.4/3400".into()],
            },
            NodeEventJson::PeerDiscovered {
                peer: peer_str.clone(),
                addrs: vec!["1.2.3.4/3400".into()],
            },
        ),
        (
            p2p::NodeEvent::PeerConnected { peer },
            NodeEventJson::PeerConnected {
                peer: peer_str.clone(),
            },
        ),
        (
            p2p::NodeEvent::PeerDisconnected { peer },
            NodeEventJson::PeerDisconnected {
                peer: peer_str.clone(),
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
            },
        ),
        (
            p2p::NodeEvent::DialFailed { peer: None, reason: "x".into() },
            NodeEventJson::DialFailed { peer: None, reason: "x".into() },
        ),
        (
            p2p::NodeEvent::ProtocolViolation { peer, reason: "x".into() },
            NodeEventJson::ProtocolViolation {
                peer: peer_str.clone(),
                reason: "x".into(),
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

#[test]
fn metrics_json_maps_from_snapshot() {
    let snapshot = p2p_swarm::MetricsSnapshot {
        dial_direct_ok: 1,
        dial_direct_fail: 2,
        relay_sessions_active: 3,
        ..Default::default()
    };
    let json = MetricsJson::from(snapshot);
    assert_eq!(json.dial_direct_ok, 1);
    assert_eq!(json.dial_direct_fail, 2);
    assert_eq!(json.relay_sessions_active, 3);
    assert_eq!(json.dial_punch_ok, 0);
}
