//! tunnel 访侧状态契约类型（gui-contract §19 逐字冻结，camelCase serde 镜像）。
//! TunnelServeStatus 为被访侧服务面（W-T2 实现）；访侧快照先按 §19 形状带
//! `serve` 字段（emit 恒带，语义=单一形状判别），值来自 W-T2 接线，接线前
//! 为默认关闭态。

use serde::{Deserialize, Serialize};

/// tunnel_open_dsh 返回（§19.1 TunnelOpenReport）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelOpenReport {
    /// 本地反代监听地址（127.0.0.1:<port>，bind(127.0.0.1:0) 随机端口）。
    pub local_addr: String,
    /// 系统浏览器入口链接（http://127.0.0.1:<local_port>/?token=<同 token>）。
    pub open_url: String,
    /// 原样透传 DSH 启动 URL 的 token（不落日志/事件，§19.3-4）。
    pub token: String,
}

/// 被访侧服务面：唯一真值源 = W-T2 装配（crate::tunnel::TunnelServeStatus）。
pub use crate::tunnel::TunnelServeStatus;

/// tunnel_status 快照 + `tunnel_status` 事件载荷（§19.2 TunnelStatusReport）。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatusReport {
    /// 存在未关闭会话。
    pub active: bool,
    /// 本地反代监听地址；无活动会话 = null。
    pub local_addr: Option<String>,
    /// 被访目标 127.0.0.1:<port>；无活动会话 = null。
    pub target: Option<String>,
    /// 会话审计（§19.2 TunnelSessionAudit 八字段全量）。
    pub sessions: Vec<TunnelSessionAudit>,
    /// 被访侧服务面（W-T2 槽位实况）。
    pub serve: TunnelServeStatus,
}

/// 会话审计记录（§19.2 八字段；outcome ∈ "open" | "ok" | 六值错误码）。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelSessionAudit {
    /// 票据 uid（16 hex，两侧日志同源）。
    pub session_id: String,
    /// 被访节点 PeerId（base58）。
    pub peer_id: String,
    /// 127.0.0.1:<port>。
    pub target: String,
    /// Unix 秒。
    pub started_at: u64,
    /// Unix 秒；未结束 = null。
    pub ended_at: Option<u64>,
    /// 记录方视角：自隧道收到。
    pub bytes_in: u64,
    /// 记录方视角：向隧道发出。
    pub bytes_out: u64,
    /// "open" | "ok" | TunnelErrorCode。
    pub outcome: String,
}

impl TunnelStatusReport {
    /// 无活动会话快照（§19.1：active=false、localAddr/target=null、sessions=[]）。
    pub fn idle() -> Self {
        Self {
            active: false,
            local_addr: None,
            target: None,
            sessions: Vec::new(),
            serve: TunnelServeStatus::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_report_serializes_camel_case() {
        let report = TunnelOpenReport {
            local_addr: "127.0.0.1:40001".into(),
            open_url: "http://127.0.0.1:40001/?token=t".into(),
            token: "t".into(),
        };
        let value = serde_json::to_value(&report).expect("serialize");
        assert_eq!(value["localAddr"], "127.0.0.1:40001");
        assert_eq!(value["openUrl"], "http://127.0.0.1:40001/?token=t");
        assert_eq!(value["token"], "t");
    }

    #[test]
    fn status_report_matches_contract_19_shapes() {
        let report = TunnelStatusReport::idle();
        let value = serde_json::to_value(&report).expect("serialize");
        assert_eq!(value["active"], false);
        assert_eq!(value["localAddr"], serde_json::Value::Null);
        assert_eq!(value["target"], serde_json::Value::Null);
        assert_eq!(value["sessions"], serde_json::json!([]));
        assert_eq!(value["serve"]["enabled"], false);
        assert_eq!(value["serve"]["allow"], serde_json::json!([]));
        assert_eq!(value["serve"]["activeSessions"], 0);
    }

    #[test]
    fn session_audit_serializes_eight_fields() {
        let audit = TunnelSessionAudit {
            session_id: "0123456789abcdef".into(),
            peer_id: "peer".into(),
            target: "127.0.0.1:3080".into(),
            started_at: 1000,
            ended_at: None,
            bytes_in: 1,
            bytes_out: 2,
            outcome: "open".into(),
        };
        let value = serde_json::to_value(&audit).expect("serialize");
        for key in [
            "sessionId",
            "peerId",
            "target",
            "startedAt",
            "endedAt",
            "bytesIn",
            "bytesOut",
            "outcome",
        ] {
            assert!(value.get(key).is_some(), "缺字段 {key}");
        }
        assert_eq!(value["endedAt"], serde_json::Value::Null);
    }
}
