//! 事件通道契约类型（gui-contract.md §2 NodeEventJson）与 NodeEvent 映射。
//!
//! 从 types.rs 拆出：该文件逼近 300 行红线（2026-09-03 source 字段加法修订）。
//! 变体名 snake_case 判别；全变体携带可选 tsMs（events::emit 统一盖戳）。

use p2p::NodeEvent;
use p2p_swarm::AddrSource;
use serde::{Deserialize, Serialize};

use super::HopKind;

/// 地址来源（契约 v5：mdns | rendezvous | manual），AddrSource 的 JSON 镜像。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Mdns,
    Rendezvous,
    Manual,
}

impl From<AddrSource> for SourceKind {
    fn from(source: AddrSource) -> Self {
        match source {
            AddrSource::Mdns => Self::Mdns,
            AddrSource::Rendezvous => Self::Rendezvous,
            AddrSource::Manual => Self::Manual,
        }
    }
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
        source: SourceKind,
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
        hop: super::HopKind,
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

impl From<NodeEvent> for NodeEventJson {
    fn from(ev: NodeEvent) -> Self {
        // ts_ms 交给 emit 出口统一盖戳，此处恒为 None
        match ev {
            NodeEvent::PeerDiscovered { peer, addrs, source } => Self::PeerDiscovered {
                peer: peer.to_string(),
                addrs,
                source: source.into(),
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
            NodeEvent::DialHop { peer, hop, ok, detail } => Self::DialHop {
                peer: peer.to_string(),
                hop: hop.into(),
                ok,
                detail,
                ts_ms: None,
            },
        }
    }
}
