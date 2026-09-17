//! GUI 契约类型的 serde 镜像（gui-contract.md §3/§11，camelCase 逐字对齐）。
//! 出厂默认端点与 apps/gui/src-tauri/src/config.rs 同源（GUI 首跑行为等价）。

use serde::{Deserialize, Serialize};

/// 出厂内置云端 bootstrap（rendezvous，QUIC 语法）。
pub fn default_bootstrap() -> Vec<String> {
    vec![
        "43.240.223.138/u3400".into(),
        "121.196.193.177/u3400".into(),
    ]
}

/// 出厂内置云端中继（relay）。
pub fn default_relay_addrs() -> Vec<String> {
    vec![
        "43.240.223.138/u3403".into(),
        "121.196.193.177/u3403".into(),
    ]
}

/// 出厂内置观测反射口（socket 语法）。
pub fn default_observation_addrs() -> Vec<String> {
    vec!["121.196.193.177:3402".into()]
}

/// 出厂默认自动绑角色（authz-a3-plan §1 S2 P1d）。
pub fn default_authz_default_role() -> String {
    "friend".to_owned()
}

/// 出厂默认远程桌面审批闸（CC2：缺省开，零行为变化）。
pub fn default_rd_require_approval() -> bool {
    true
}

/// 出厂默认远程桌面初始质量档（CC2：合法域 1..=60，缺省 15）。
pub fn default_rd_fps() -> u8 {
    15
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
    /// 仅局域网模式（F8）：true 时启动不连任何公共设施，仅局域网发现与直连。
    #[serde(default)]
    pub lan_only: bool,
    /// 加好友自动绑定角色（P1d）：内建或自定义角色 id；空串 = 禁用自动绑。
    /// GUI 侧类型尚未含此字段（S3 接入），serde default 保证旧配置文件可读。
    #[serde(default = "default_authz_default_role")]
    pub authz_default_role: String,
    /// 远程桌面审批闸缺省（CC2）：rd_host_start 未显式指定时取本值。
    #[serde(default = "default_rd_require_approval")]
    pub rd_require_approval: bool,
    /// 远程桌面初始质量档 fps（合法域 1..=60）；缺省 15 零行为变化。
    #[serde(default = "default_rd_fps")]
    pub rd_fps: u8,
    /// tunnel serve 白名单（"127.0.0.1:<port>" 字面量）；GUI 写通，CLI 读同文件。
    #[serde(default)]
    pub tunnel_serve_allow: Vec<String>,
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
            lan_only: false,
            authz_default_role: default_authz_default_role(),
            rd_require_approval: default_rd_require_approval(),
            rd_fps: default_rd_fps(),
            tunnel_serve_allow: Vec::new(),
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
        assert!(cfg.rd_require_approval, "CC2 缺省 true");
        assert_eq!(cfg.rd_fps, 15, "CC2 缺省 15");
        assert!(cfg.tunnel_serve_allow.is_empty(), "CC2 缺省空");
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
        assert!(json.get("rdRequireApproval").is_some(), "CC2 camelCase 键");
        assert!(json.get("rdFps").is_some());
        assert!(json.get("tunnelServeAllow").is_some());
    }

    /// CC2：三字段缺省 true/15/空（旧配置零行为变化）；显式值 camelCase 往返保真。
    #[test]
    fn config_rd_tunnel_fields_default_and_roundtrip() {
        let cfg: GuiConfig = serde_json::from_str("{}").unwrap();
        assert!(cfg.rd_require_approval);
        assert_eq!(cfg.rd_fps, 15);
        assert!(cfg.tunnel_serve_allow.is_empty());
        let json = serde_json::to_value(&GuiConfig {
            rd_require_approval: false,
            rd_fps: 60,
            tunnel_serve_allow: vec!["127.0.0.1:5900".into()],
            ..GuiConfig::default()
        })
        .unwrap();
        assert_eq!(json["rdRequireApproval"], serde_json::json!(false));
        assert_eq!(json["rdFps"], serde_json::json!(60));
        assert_eq!(
            json["tunnelServeAllow"],
            serde_json::json!(["127.0.0.1:5900"])
        );
        let back: GuiConfig = serde_json::from_value(json).unwrap();
        assert!(!back.rd_require_approval, "显式值不被默认覆盖");
        assert_eq!(back.rd_fps, 60);
        assert_eq!(back.tunnel_serve_allow, vec!["127.0.0.1:5900".to_owned()]);
    }

    #[test]
    fn config_lan_only_defaults_false_and_roundtrips() {
        let cfg: GuiConfig = serde_json::from_str("{}").unwrap();
        assert!(!cfg.lan_only, "缺省 lan-only 必须 false：零行为变化");
        let cfg: GuiConfig = serde_json::from_str("{\"lanOnly\":true}").unwrap();
        assert!(cfg.lan_only);
        let json = serde_json::to_value(&cfg).unwrap();
        assert_eq!(
            json["lanOnly"],
            serde_json::json!(true),
            "camelCase 往返保真"
        );
        let back: GuiConfig = serde_json::from_value(
            serde_json::to_value(GuiConfig::default()).unwrap_or(serde_json::Value::Null),
        )
        .unwrap();
        assert!(!back.lan_only, "旧配置文件（无 lanOnly 字段）读取不受影响");
    }

    /// P1d：缺省 friend；旧配置无此字段读出缺省；空串显式禁用可往返。
    #[test]
    fn config_authz_default_role_defaults_and_roundtrips() {
        let cfg: GuiConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg.authz_default_role, "friend", "缺省 friend");
        let cfg: GuiConfig = serde_json::from_str(r#"{"authzDefaultRole":""}"#).unwrap();
        assert_eq!(cfg.authz_default_role, "", "空串 = 显式禁用自动绑");
        let cfg: GuiConfig = serde_json::from_str(r#"{"authzDefaultRole":"operator"}"#).unwrap();
        assert_eq!(cfg.authz_default_role, "operator", "可设任意角色 id");
        let json = serde_json::to_value(GuiConfig::default()).unwrap();
        assert_eq!(json["authzDefaultRole"], serde_json::json!("friend"));
    }
}
