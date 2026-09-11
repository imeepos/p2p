//! tunnel 访侧状态契约类型（冻结契约 §7：命令/事件字段面，camelCase serde 镜像）。
//! `visited*` 三字段是被访侧（W-T2，GUI 进程内 responder）的取数位：其 Rust
//! 内部 API 落地后接线；接线前恒为 None（前端渲染为「未装配」）。

use serde::{Deserialize, Serialize};

/// tunnel_open_dsh 返回（契约 §7：local_addr / open_url / token）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelOpenResult {
    /// 本地反代监听地址（127.0.0.1:<port>）。
    pub local_addr: String,
    /// 系统浏览器入口链接（同 token 原样透传）。
    pub open_url: String,
    /// DSH 启动 URL 解析出的 token（open_url 的 query 部分）。
    pub token: String,
}

/// tunnel_status 快照 + `tunnel_status` 事件载荷。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TunnelStatus {
    /// 访侧反代是否已开启。
    pub open: bool,
    /// 开启后：本地监听地址。
    pub local_addr: Option<String>,
    /// 开启后：浏览器入口链接。
    pub open_url: Option<String>,
    /// 开启后：隧道目标（127.0.0.1:<dsh_port>）。
    pub target: Option<String>,
    /// 开启后：被访节点 PeerId。
    pub peer: Option<String>,
    /// 活动转发连接数（浏览器⇄隧道 1:1）。
    pub active_conns: u32,
    /// 最近一次失败原因（开启失败/转发面不可用时）。
    pub last_error: Option<String>,
    /// 被访侧 responder 是否开启（W-T2 接线前 None）。
    pub visited_open: Option<bool>,
    /// 被访侧白名单（W-T2 接线前 None）。
    pub visited_allowlist: Option<Vec<String>>,
    /// 被访侧活动会话数（W-T2 接线前 None）。
    pub visited_active_sessions: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_result_serializes_camel_case() {
        let result = TunnelOpenResult {
            local_addr: "127.0.0.1:40001".into(),
            open_url: "http://127.0.0.1:40001/?token=t".into(),
            token: "t".into(),
        };
        let value = serde_json::to_value(&result).expect("serialize");
        assert_eq!(value["localAddr"], "127.0.0.1:40001");
        assert_eq!(value["openUrl"], "http://127.0.0.1:40001/?token=t");
    }

    #[test]
    fn status_serializes_snake_fields_to_camel() {
        let status = TunnelStatus {
            open: true,
            active_conns: 2,
            visited_open: Some(false),
            ..Default::default()
        };
        let value = serde_json::to_value(&status).expect("serialize");
        assert_eq!(value["activeConns"], 2);
        assert_eq!(value["visitedOpen"], false);
        assert_eq!(value["lastError"], serde_json::Value::Null);
    }
}
