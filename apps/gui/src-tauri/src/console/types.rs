//! in-process pump 状态契约类型（gui-contract.md §15）。
//! 字段名与契约逐字对齐（camelCase）；serde default 容忍缺省字段。
//! 进程内装配后无外部进程监督语义：只有 connecting/connected/disconnected 三态。

use serde::{Deserialize, Serialize};

use acp_pump::PumpHandle;

/// pump 状态（契约 §15：connecting | connected | disconnected）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AcpConsolePhase {
    #[default]
    Connecting,
    Connected,
    Disconnected,
}

/// pump 状态快照（契约 §15）。连接面字段仅 connected 携带；Option 序列化为 null。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AcpConsoleStatus {
    pub phase: AcpConsolePhase,
    /// connected 后：ws://127.0.0.1:<port>
    pub ws_url: Option<String>,
    /// connected 后：console WS 鉴权 token
    pub token: Option<String>,
    /// connected 后：pump status HTTP 地址
    pub status_url: Option<String>,
    /// 最近一次失败原因（可读中文/英文；干净收尾为 None）
    pub last_error: Option<String>,
}

impl AcpConsoleStatus {
    /// 装配初值：Pump::start 尚未返回。
    pub fn connecting() -> Self {
        Self {
            phase: AcpConsolePhase::Connecting,
            ..Self::default()
        }
    }

    /// 就绪：连接面填充（端口/token 来自进程内句柄，无 stdout 解析）。
    pub fn connected(handle: &PumpHandle) -> Self {
        Self {
            phase: AcpConsolePhase::Connected,
            ws_url: Some(format!("ws://{}", handle.ws_addr)),
            token: Some(handle.token.clone()),
            status_url: Some(format!("http://{}", handle.status_addr)),
            last_error: None,
        }
    }

    /// 断开：连接面清空（旧 WS/token 已失效）；err 留最近失败原因。
    pub fn disconnected(err: Option<String>) -> Self {
        Self {
            phase: AcpConsolePhase::Disconnected,
            ws_url: None,
            token: None,
            status_url: None,
            last_error: err,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_serializes_lowercase() {
        assert_eq!(
            serde_json::to_value(AcpConsolePhase::Connected).expect("serialize"),
            "connected"
        );
        let phase: AcpConsolePhase =
            serde_json::from_value(serde_json::json!("connecting")).expect("deserialize");
        assert_eq!(phase, AcpConsolePhase::Connecting);
    }
}
