//! acp status 子命令（INLINE-ACP-PUMP T6）：查询运行中泵的 status HTTP 快照，
//! 与 GUI acp_console_status 命令同源同词汇（/status 端点，Bearer 鉴权）。
//! 本层只做 HTTP GET 与呈现，不复制泵实现。

use std::time::Duration;

use clap::Args;
use serde::Serialize;

use crate::error::{CliError, CliResult};
use crate::output;

#[derive(Args)]
pub struct StatusArgs {
    /// pump status HTTP 地址（http://127.0.0.1:<port>，ready 行 status 字段）
    #[arg(long, value_name = "URL")]
    pub status_url: String,
    /// console 鉴权 token（ready 行 token 字段）
    #[arg(long, value_name = "TOKEN")]
    pub token: String,
    /// 输出结构化 JSON（原样透出 /status 响应体，含连接面）
    #[arg(long)]
    pub json: bool,
}

#[derive(Serialize)]
pub struct StatusReport {
    pub phase: String,
    pub ws_url: Option<String>,
    /// 人工视图脱敏：只透出 token 是否已配置，原文走 --json
    pub token_set: bool,
    pub status_url: Option<String>,
    pub last_error: Option<String>,
}

pub async fn run(args: StatusArgs) -> CliResult<()> {
    let raw = fetch_snapshot(&args).await?;
    if args.json {
        println!("{raw}");
        return Ok(());
    }
    let report = render_report(&raw)?;
    let text = render_text(&report);
    output::emit(false, &report, &text)
}

/// GET {status_url}/status，Bearer 鉴权；非 2XX/不可达结构化报错不静默。
async fn fetch_snapshot(args: &StatusArgs) -> CliResult<String> {
    let url = endpoint_url(&args.status_url)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| CliError::Runtime(format!("http client: {e}")))?;
    let resp = client
        .get(&url)
        .bearer_auth(&args.token)
        .send()
        .await
        .map_err(|e| CliError::Runtime(format!("status 端点不可达 {url}: {e}")))?;
    let code = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| CliError::Runtime(format!("status 响应读取失败: {e}")))?;
    if !code.is_success() {
        return Err(CliError::Runtime(format!("status 端点返回 {code}: {body}")));
    }
    Ok(body)
}

/// 地址规范化：补 /status 路径；坏 URL 显式报错（fail-fast）。
fn endpoint_url(base: &str) -> CliResult<String> {
    let trimmed = base.trim_end_matches('/');
    let parsed = reqwest::Url::parse(&format!("{trimmed}/status"))
        .map_err(|e| CliError::Runtime(format!("--status-url 非法 {base}: {e}")))?;
    Ok(parsed.to_string())
}

/// 响应体 → 人工视图（token 脱敏；字段缺失容忍，坏 JSON 显式报错）。
fn render_report(raw: &str) -> CliResult<StatusReport> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| CliError::Runtime(format!("status 响应非 JSON: {e}")))?;
    Ok(StatusReport {
        phase: value
            .get("phase")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        ws_url: value
            .get("wsUrl")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        token_set: value
            .get("token")
            .and_then(serde_json::Value::as_str)
            .is_some(),
        status_url: value
            .get("statusUrl")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        last_error: value
            .get("lastError")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
    })
}

fn render_text(report: &StatusReport) -> String {
    let mut lines = vec![format!("phase: {}", report.phase)];
    if let Some(ws) = &report.ws_url {
        lines.push(format!("ws: {ws}"));
    }
    if let Some(status) = &report.status_url {
        lines.push(format!("status: {status}"));
    }
    lines.push(format!(
        "token: {}",
        if report.token_set {
            "已配置"
        } else {
            "缺失"
        }
    ));
    if let Some(err) = &report.last_error {
        lines.push(format!("lastError: {err}"));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_url_appends_status_path() {
        assert_eq!(
            endpoint_url("http://127.0.0.1:9102/").unwrap(),
            "http://127.0.0.1:9102/status"
        );
        assert!(endpoint_url("not a url").is_err());
    }

    const SNAPSHOT: &str = r#"{"phase":"connected","wsUrl":"ws://127.0.0.1:9101","token":"t","statusUrl":"http://127.0.0.1:9102"}"#;

    #[test]
    fn render_maps_fields_and_masks_token() {
        let report = render_report(SNAPSHOT).unwrap();
        assert_eq!(report.phase, "connected");
        assert_eq!(report.ws_url.as_deref(), Some("ws://127.0.0.1:9101"));
        assert!(report.token_set, "token 存在只透出布尔");
        let text = render_text(&report);
        assert!(
            text.contains("phase: connected") && !text.contains("\"t\""),
            "{text}"
        );
    }

    #[test]
    fn render_tolerates_missing_fields_and_rejects_bad_json() {
        let report = render_report(r#"{"phase":"disconnected"}"#).unwrap();
        assert!(report.ws_url.is_none() && !report.token_set);
        assert!(render_report("not-json").is_err());
    }
}
