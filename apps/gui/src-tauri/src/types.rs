//! gui-contract.md §2/§3 契约类型的 serde 镜像。
//!
//! 字段名与契约逐字对齐（camelCase），Option 序列化为 null；事件类型（NodeEventJson）
//! 见子模块 node_event——拆出原委见该文件头注释，此处 re-export 保持 types:: 路径稳定。

use p2p_swarm::MetricsSnapshot;
use serde::{Deserialize, Serialize};

#[cfg(test)]
pub(crate) mod testing;
mod node_event;

pub use node_event::{NodeEventJson, SourceKind};

/// 节点启停配置（契约 §3 GuiConfig）。
///
/// 字段级 serde default 使旧配置文件缺失字段时逐字段补出厂默认
/// （含云端端点），已有字段永不覆盖；完整默认见 config.rs。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiConfig {
    /// 0 = 随机端口。
    #[serde(default)]
    pub quic_port: u16,
    /// 0 = 随机端口。
    #[serde(default)]
    pub tcp_port: u16,
    #[serde(default = "crate::config::default_true")]
    pub enable_mdns: bool,
    /// 默认 app 数据目录下 p2p-data。
    #[serde(default = "crate::config::default_data_dir")]
    pub data_dir: String,
    /// rendezvous 地址，语法同 §6："ip/u端口"（QUIC）或 "ip/t端口"（TCP）。
    #[serde(default = "crate::config::default_bootstrap")]
    pub bootstrap: Vec<String>,
    #[serde(default = "crate::config::default_relay_addrs")]
    pub relay_addrs: Vec<String>,
    #[serde(default)]
    pub advertised_addrs: Vec<String>,
    #[serde(default)]
    pub observation_port: Option<u16>,
    #[serde(default = "crate::config::default_observation_addrs")]
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

impl From<p2p_swarm::DialHop> for HopKind {
    fn from(hop: p2p_swarm::DialHop) -> Self {
        match hop {
            p2p_swarm::DialHop::Direct => Self::Direct,
            p2p_swarm::DialHop::Punch => Self::Punch,
            p2p_swarm::DialHop::Relay => Self::Relay,
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

#[cfg(test)]
mod event_tests;

#[cfg(test)]
mod tests;
