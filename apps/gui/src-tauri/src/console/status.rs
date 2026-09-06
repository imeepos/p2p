//! 托管状态契约类型（gui-contract.md §15 AcpConsoleStatus）与 stdout ready 行解析。
//!
//! 字段名与契约逐字对齐（camelCase）；serde default 容忍缺省字段（§15 缺省容忍语义）。

use serde::{Deserialize, Serialize};

/// 托管 phase（契约 §15：starting | ready | restarting | failed | unavailable | stopped）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AcpConsolePhase {
    #[default]
    Starting,
    Ready,
    Restarting,
    Failed,
    Unavailable,
    Stopped,
}

/// 托管状态快照（契约 §15）。连接面字段仅 ready 后携带；Option 序列化为 null。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AcpConsoleStatus {
    pub phase: AcpConsolePhase,
    /// ready 后：ws://127.0.0.1:<port>
    pub ws_url: Option<String>,
    /// ready 后：console WS 鉴权 token
    pub token: Option<String>,
    /// ready 后：console status HTTP 地址
    pub status_url: Option<String>,
    /// ready 后：agent admin HTTP 地址（ready 行携带才填）
    pub admin_url: Option<String>,
    /// 已自动重启次数
    pub restarts: u64,
    /// 最近一次失败原因（可读中文）
    pub last_error: Option<String>,
}

impl AcpConsoleStatus {
    /// 监督启动初值。
    pub(crate) fn initial() -> Self {
        Self {
            phase: AcpConsolePhase::Starting,
            ..Self::default()
        }
    }

    /// 定位失败（契约 §15：unavailable 显式留 lastError，不阻断 GUI 主功能）。
    pub(crate) fn unavailable(last_error: impl Into<String>) -> Self {
        Self {
            phase: AcpConsolePhase::Unavailable,
            last_error: Some(last_error.into()),
            ..Self::default()
        }
    }

    /// 就绪行装配：连接面填充；restarts 累计、lastError 保留最近失败原因。
    pub(crate) fn apply_ready(&mut self, ready: &ReadyLine) {
        self.phase = AcpConsolePhase::Ready;
        self.ws_url = ready.ws.as_deref().map(ws_url);
        self.token = ready.token.clone();
        self.status_url = ready.status.as_deref().map(http_url);
        self.admin_url = ready.admin.as_deref().map(http_url);
    }

    /// 崩溃/失败后的非 ready 态迁移：连接面清空（旧 WS/token 已失效）。
    pub(crate) fn mark_down(&mut self, phase: AcpConsolePhase, restarts: u64, last_error: &str) {
        self.phase = phase;
        self.restarts = restarts;
        self.last_error = Some(last_error.to_string());
        self.clear_face();
    }

    /// 收尾态：连接面清空，restarts/lastError 保留历史。
    pub(crate) fn mark_stopped(&mut self) {
        self.phase = AcpConsolePhase::Stopped;
        self.clear_face();
    }

    pub(crate) fn clear_face(&mut self) {
        self.ws_url = None;
        self.token = None;
        self.status_url = None;
        self.admin_url = None;
    }
}

fn ws_url(host_port: &str) -> String {
    format!("ws://{host_port}")
}

fn http_url(host_port: &str) -> String {
    format!("http://{host_port}")
}

/// acp-console stdout 就绪行（apps/acp-console/README.md「stdout JSON 行契约」）：
/// kind=ready 载荷 {ws, status, token, peer}；admin 为契约 §15 adminUrl 预留可选项。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ReadyLine {
    kind: String,
    ws: Option<String>,
    status: Option<String>,
    token: Option<String>,
    admin: Option<String>,
    #[allow(dead_code)]
    peer: Option<String>,
}

/// stdout 单行解析：kind=ready 返回 Some(载荷)；其余合法 JSON 对象（state/discovery/
/// share-connect 等协议行）返回 None 不参与就绪；非法行返回 Err（计失败观测并留痕）。
pub(crate) fn parse_line(line: &str) -> Result<Option<ReadyLine>, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| format!("acp-console stdout 行非法 JSON（{e}）：{trimmed}"))?;
    if !value.is_object() {
        return Err(format!("acp-console stdout 行非 JSON 对象：{trimmed}"));
    }
    let kind = value
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if kind != "ready" {
        return Ok(None);
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(|e| format!("acp-console ready 行字段类型非法（{e}）：{trimmed}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ready_line_full() {
        let ready = parse_line(
            r#"{"kind":"ready","ws":"127.0.0.1:9101","status":"127.0.0.1:9102","token":"tk","peer":"p"}"#,
        )
        .expect("合法 ready 行必须解析")
        .expect("kind=ready 必须返回载荷");
        assert_eq!(ready.ws.as_deref(), Some("127.0.0.1:9101"));
        assert_eq!(ready.status.as_deref(), Some("127.0.0.1:9102"));
        assert_eq!(ready.token.as_deref(), Some("tk"));
        assert_eq!(ready.admin, None, "admin 为可选字段");
    }

    #[test]
    fn parse_tolerates_missing_fields() {
        let ready = parse_line(r#"{"kind":"ready","token":"tk"}"#)
            .expect("缺省字段必须容忍")
            .expect("ready 载荷");
        assert_eq!(ready.ws, None);
        assert_eq!(ready.token.as_deref(), Some("tk"));
    }

    #[test]
    fn parse_skips_other_protocol_lines() {
        for line in [
            r#"{"kind":"state","phase":"online","since_unix_ms":1}"#,
            r#"{"kind":"discovery","peers":[]}"#,
            r#"{"kind":"share-connect","peer":"p","ok":true}"#,
        ] {
            assert!(
                parse_line(line).expect("协议行不得报错").is_none(),
                "非 ready 行不参与就绪: {line}"
            );
        }
    }

    #[test]
    fn parse_rejects_invalid_lines() {
        assert!(parse_line("not-json").is_err(), "非 JSON 必须报错");
        assert!(parse_line("[1,2]").is_err(), "非对象必须报错");
    }

    #[test]
    fn apply_ready_fills_face_and_keeps_history() {
        let mut status = AcpConsoleStatus::unavailable("定位失败");
        status.restarts = 2;
        let ready = parse_line(r#"{"kind":"ready","ws":"127.0.0.1:1","token":"t"}"#)
            .expect("解析")
            .expect("ready");
        status.apply_ready(&ready);
        assert_eq!(status.phase, AcpConsolePhase::Ready);
        assert_eq!(status.ws_url.as_deref(), Some("ws://127.0.0.1:1"));
        assert_eq!(status.token.as_deref(), Some("t"));
        assert!(status.status_url.is_none() && status.admin_url.is_none());
        assert_eq!(status.restarts, 2, "restarts 累计不清零");
        assert_eq!(status.last_error.as_deref(), Some("定位失败"));
    }
}
