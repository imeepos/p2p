//! 契约 §3 数据类型 serde 单测：camelCase 逐字段断言 + roundtrip。

use super::{roundtrip, sample_config, GuiConfig, MetricsJson, NodeStatus, PingOutcome, DialReport, DialHopJson, HopKind};
use serde_json::{json, Value};

#[test]
fn gui_config_camel_case_roundtrip() {
    roundtrip(
        &sample_config(),
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
        &GuiConfig {
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
        &status,
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
        &metrics,
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
        &report,
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
    roundtrip(&ok, json!({ "ok": true, "rttMs": 12, "hops": [], "error": null }));
    let failed = PingOutcome {
        ok: false,
        rtt_ms: None,
        hops: Vec::new(),
        error: Some("超时".into()),
    };
    roundtrip(
        &failed,
        json!({ "ok": false, "rttMs": null, "hops": [], "error": "超时" }),
    );
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
