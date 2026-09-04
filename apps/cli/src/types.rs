//! GUI 契约类型的 serde 镜像（gui-contract.md §3/§11，camelCase 逐字对齐）。
//! 出厂默认端点与 apps/gui/src-tauri/src/config.rs 同源（GUI 首跑行为等价）。

use serde::{Deserialize, Serialize};

/// 出厂内置云端 bootstrap（rendezvous，QUIC 语法）。
pub fn default_bootstrap() -> Vec<String> {
    vec!["43.240.223.138/u3400".into(), "121.196.193.177/u3400".into()]
}

/// 出厂内置云端中继（relay）。
pub fn default_relay_addrs() -> Vec<String> {
    vec!["43.240.223.138/u3403".into(), "121.196.193.177/u3403".into()]
}

/// 出厂内置观测反射口（socket 语法）。
pub fn default_observation_addrs() -> Vec<String> {
    vec!["121.196.193.177:3402".into()]
}

/// 节点启停配置（契约 §3 GuiConfig）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GuiConfig {
    #[serde(default)]
    pub quic_port: u16,
    #[serde(default)]
    pub tcp_port: u16,
    #[serde(default = "default_true")]
    pub enable_mdns: bool,
    #[serde(default)]
    pub data_dir: String,
    #[serde(default = "default_bootstrap")]
    pub bootstrap: Vec<String>,
    #[serde(default = "default_relay_addrs")]
    pub relay_addrs: Vec<String>,
    #[serde(default)]
    pub advertised_addrs: Vec<String>,
    #[serde(default)]
    pub observation_port: Option<u16>,
    #[serde(default = "default_observation_addrs")]
    pub observation_addrs: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            quic_port: 0,
            tcp_port: 0,
            enable_mdns: true,
            data_dir: String::new(),
            bootstrap: default_bootstrap(),
            relay_addrs: default_relay_addrs(),
            advertised_addrs: Vec::new(),
            observation_port: None,
            observation_addrs: default_observation_addrs(),
        }
    }
}

/// 节点状态快照（契约 §3 NodeStatus）。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStatus {
    pub running: bool,
    pub peer_id: Option<String>,
    pub listen_addrs: Vec<String>,
    pub uptime_secs: u64,
    pub started_at_ms: Option<u64>,
    pub config: GuiConfig,
}

/// 节点资料（契约 §11 NodeProfile）。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NodeProfile {
    pub name: String,
    pub description: String,
    pub avatar: Option<String>,
}

/// 降级链一跳类型（"direct" | "punch" | "relay"）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HopKind {
    Direct,
    Punch,
    Relay,
}

impl From<p2p_swarm::DialHop> for HopKind {
    fn from(hop: p2p_swarm::DialHop) -> Self {
        match hop {
            p2p_swarm::DialHop::Direct => Self::Direct,
            p2p_swarm::DialHop::Punch => Self::Punch,
            p2p_swarm::DialHop::Relay => Self::Relay,
        }
    }
}

/// 逐跳报告（契约 §3）。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialHopJson {
    pub hop: HopKind,
    pub ok: bool,
    pub detail: String,
}

/// 拨号报告（契约 §3）。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialReport {
    pub peer: String,
    pub hops: Vec<DialHopJson>,
    pub ok: bool,
    pub total_ms: u64,
}

/// echo 测距结果（契约 §3）。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingOutcome {
    pub ok: bool,
    pub rtt_ms: Option<u64>,
    pub hops: Vec<DialHopJson>,
    pub error: Option<String>,
}

/// 运行时指标快照（契约 v2 MetricsJson）：未运行时全零，字段与 GUI 逐字同形。
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

impl From<p2p_swarm::MetricsSnapshot> for MetricsJson {
    fn from(m: p2p_swarm::MetricsSnapshot) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_match_gui_factory() {
        let cfg = GuiConfig::default();
        assert_eq!(cfg.quic_port, 0);
        assert!(cfg.enable_mdns);
        assert_eq!(cfg.bootstrap, default_bootstrap());
        assert_eq!(cfg.relay_addrs, default_relay_addrs());
        assert_eq!(cfg.observation_addrs, default_observation_addrs());
    }

    #[test]
    fn config_json_is_camel_case_and_field_defaults_fill_gaps() {
        let cfg: GuiConfig = serde_json::from_str("{\"quicPort\":3400}").unwrap();
        assert_eq!(cfg.quic_port, 3400);
        assert!(cfg.enable_mdns, "缺失字段补默认");
        assert_eq!(cfg.bootstrap, default_bootstrap());
        let json = serde_json::to_value(GuiConfig::default()).unwrap();
        assert!(json.get("quicPort").is_some());
        assert!(json.get("relayAddrs").is_some());
    }
}
