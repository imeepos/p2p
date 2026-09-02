//! gui-contract.md §2/§3 契约类型的 serde 镜像。
//!
//! 字段名与契约逐字对齐（camelCase），Option 序列化为 null；事件为 type 判别联合，
//! 全变体携带可选 tsMs（发射时由 events::emit 统一盖戳，无值序列化时省略）。

use p2p::NodeEvent;
use p2p_swarm::{DialHop, MetricsSnapshot};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::Value;

/// 节点启停配置（契约 §3 GuiConfig）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiConfig {
    /// 0 = 随机端口。
    pub quic_port: u16,
    /// 0 = 随机端口。
    pub tcp_port: u16,
    pub enable_mdns: bool,
    /// 默认 app 数据目录下 p2p-data。
    pub data_dir: String,
    /// rendezvous 地址，语法同 §6："ip/u端口"（QUIC）或 "ip/t端口"（TCP）。
    pub bootstrap: Vec<String>,
    pub relay_addrs: Vec<String>,
    pub advertised_addrs: Vec<String>,
    pub observation_port: Option<u16>,
    pub observation_addrs: Vec<String>,
}

/// 节点状态快照（契约 §3 NodeStatus）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStatus {
    pub running: bool,
    /// base58(sha256(pubkey))；未运行 null。
    pub peer_id: Option<String>,
    pub listen_addrs: Vec<String>,
    pub uptime_secs: u64,
    pub started_at_ms: Option<u64>,
    /// 运行中 = 生效配置；未运行 = 持久化配置。
    pub config: GuiConfig,
}

/// 运行时指标（契约 §3 MetricsJson，MetricsSnapshot 的 JSON 镜像）。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsJson {
    pub dial_direct_ok: u64,
    pub dial_direct_fail: u64,
    pub dial_punch_ok: u64,
    pub dial_punch_fail: u64,
    pub dial_relay_ok: u64,
    pub dial_relay_fail: u64,
    pub addr_dial_failures: u64,
    pub relay_reconnects: u64,
    pub gate_denials_total: u64,
    pub active_connections: u64,
    pub relay_sessions_active: u64,
}

impl From<MetricsSnapshot> for MetricsJson {
    fn from(m: MetricsSnapshot) -> Self {
        Self {
            dial_direct_ok: m.dial_direct_ok,
            dial_direct_fail: m.dial_direct_fail,
            dial_punch_ok: m.dial_punch_ok,
            dial_punch_fail: m.dial_punch_fail,
            dial_relay_ok: m.dial_relay_ok,
            dial_relay_fail: m.dial_relay_fail,
            addr_dial_failures: m.addr_dial_failures,
            relay_reconnects: m.relay_reconnects,
            gate_denials_total: m.gate_denials_total,
            active_connections: m.active_connections,
            relay_sessions_active: m.relay_sessions_active,
        }
    }
}

/// 降级链一跳的类型（契约 §3："direct" | "punch" | "relay"）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HopKind {
    Direct,
    Punch,
    Relay,
}

impl From<DialHop> for HopKind {
    fn from(hop: DialHop) -> Self {
        match hop {
            DialHop::Direct => Self::Direct,
            DialHop::Punch => Self::Punch,
            DialHop::Relay => Self::Relay,
        }
    }
}

/// 逐跳报告（契约 §3 DialHopJson）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialHopJson {
    pub hop: HopKind,
    pub ok: bool,
    pub detail: String,
}

/// 拨号报告（契约 §3 DialReport）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialReport {
    pub peer: String,
    pub hops: Vec<DialHopJson>,
    pub ok: bool,
    pub total_ms: u64,
}

/// echo 测距结果（契约 §3 PingOutcome）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingOutcome {
    pub ok: bool,
    pub rtt_ms: Option<u64>,
    pub hops: Vec<DialHopJson>,
    pub error: Option<String>,
}

/// 节点事件（契约 §2 NodeEventJson）：app.emit("node-event") 的载荷。
///
/// 变体名按 snake_case 判别；应用级事件 node_started / node_stopped / node_error
/// 由桥接层自产。各变体的可选 ts_ms 在 events::emit 出口统一盖发射时刻戳。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeEventJson {
    PeerDiscovered {
        peer: String,
        addrs: Vec<String>,
        #[serde(rename = "tsMs", skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    PeerConnected {
        peer: String,
        #[serde(rename = "tsMs", skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    PeerDisconnected {
        peer: String,
        #[serde(rename = "tsMs", skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    ListenFailed {
        addr: String,
        reason: String,
        #[serde(rename = "tsMs", skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    DialFailed {
        peer: Option<String>,
        reason: String,
        #[serde(rename = "tsMs", skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    ProtocolViolation {
        peer: String,
        reason: String,
        #[serde(rename = "tsMs", skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    DialHop {
        peer: String,
        hop: HopKind,
        ok: bool,
        detail: String,
        #[serde(rename = "tsMs", skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    NodeStarted {
        listen_addrs: Vec<String>,
        #[serde(rename = "tsMs", skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    NodeStopped {
        #[serde(rename = "tsMs", skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
    NodeError {
        reason: String,
        #[serde(rename = "tsMs", skip_serializing_if = "Option::is_none")]
        ts_ms: Option<u64>,
    },
}

impl NodeEventJson {
    /// 发射前盖发射时刻毫秒戳；emit 出口统一调用（契约 §2 可选 tsMs）。
    pub fn stamped(mut self, ts_ms: u64) -> Self {
        let ts = Some(ts_ms);
        match &mut self {
            Self::PeerDiscovered { ts_ms, .. }
            | Self::PeerConnected { ts_ms, .. }
            | Self::PeerDisconnected { ts_ms, .. }
            | Self::ListenFailed { ts_ms, .. }
            | Self::DialFailed { ts_ms, .. }
            | Self::ProtocolViolation { ts_ms, .. }
            | Self::DialHop { ts_ms, .. }
            | Self::NodeStarted { ts_ms, .. }
            | Self::NodeStopped { ts_ms }
            | Self::NodeError { ts_ms, .. } => *ts_ms = ts,
        }
        self
    }
}

#[cfg(test)]
mod event_tests;

#[cfg(test)]
mod tests;

impl From<NodeEvent> for NodeEventJson {
    fn from(ev: NodeEvent) -> Self {
        // ts_ms 交给 emit 出口统一盖戳，此处恒为 None
        match ev {
            NodeEvent::PeerDiscovered { peer, addrs } => Self::PeerDiscovered {
                peer: peer.to_string(),
                addrs,
                ts_ms: None,
            },
            NodeEvent::PeerConnected { peer } => Self::PeerConnected {
                peer: peer.to_string(),
                ts_ms: None,
            },
            NodeEvent::PeerDisconnected { peer } => Self::PeerDisconnected {
                peer: peer.to_string(),
                ts_ms: None,
            },
            NodeEvent::ListenFailed { addr, reason } => Self::ListenFailed {
                addr,
                reason,
                ts_ms: None,
            },
            NodeEvent::DialFailed { peer, reason } => Self::DialFailed {
                peer: peer.map(|p| p.to_string()),
                reason,
                ts_ms: None,
            },
            NodeEvent::ProtocolViolation { peer, reason } => Self::ProtocolViolation {
                peer: peer.to_string(),
                reason,
                ts_ms: None,
            },
            NodeEvent::DialHop {
                peer,
                hop,
                ok,
                detail,
            } => Self::DialHop {
                peer: peer.to_string(),
                hop: hop.into(),
                ok,
                detail,
                ts_ms: None,
            },
        }
    }
}
#[cfg(test)]
pub(crate) fn roundtrip<T>(value: &T, raw: Value)
where
    T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let encoded = serde_json::to_value(value).expect("序列化");
    assert_eq!(encoded, raw, "序列化字段与契约不一致");
    let decoded: T = serde_json::from_value(raw).expect("反序列化");
    assert_eq!(&decoded, value, "roundtrip 不保真");
}

#[cfg(test)]
pub(crate) fn sample_config() -> GuiConfig {
    GuiConfig {
        quic_port: 3400,
        tcp_port: 3401,
        enable_mdns: true,
        data_dir: "/data/p2p-data".into(),
        bootstrap: vec!["1.2.3.4/u3400".into(), "1.2.3.4/t3401".into()],
        relay_addrs: vec!["5.6.7.8/u3400".into()],
        advertised_addrs: vec!["9.9.9.9/u4000".into()],
        observation_port: Some(3402),
        observation_addrs: vec!["1.2.3.4:3402".into()],
    }
}
