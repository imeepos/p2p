//! Claude 上游适配（§5.2）：OpenAI chat completions 请求体 → Claude /v1/messages，
//! 响应翻译状态机见 [response]。与 [crate::upstream_http] 各自独立；测试不出网。

use futures::StreamExt;
use serde_json::{json, Value};

use crate::upstream::{SseByteStream, Upstream, UpstreamCall, UpstreamFailure};

mod response;

/// 请求翻译拒绝：一期不支持的结构化原因，不静默字符串化（§5.2）。
#[derive(Debug, thiserror::Error)]
pub enum TranslateError {
    #[error("missing {0}")]
    MissingField(&'static str),
    #[error("unsupported {0}")]
    ToolUnsupported(&'static str),
    #[error("bad shape: {0}")]
    BadShape(String),
}
/// OpenAI 无 Claude 等价物、需显式省略并告警的请求字段。
const UNSUPPORTED_KEYS: &[&str] = &[
    "frequency_penalty",
    "presence_penalty",
    "logprobs",
    "n",
    "seed",
    "response_format",
];
/// OpenAI chat completions 请求体 → Claude /v1/messages 请求体。
pub fn translate_request(body: &Value) -> Result<Value, TranslateError> {
    let obj = body
        .as_object()
        .ok_or_else(|| TranslateError::BadShape("body is not an object".into()))?;
    if obj.contains_key("tools") || obj.contains_key("tool_choice") {
        return Err(TranslateError::ToolUnsupported("tools/tool_choice"));
    }
    let model = obj
        .get("model")
        .and_then(Value::as_str)
        .ok_or(TranslateError::MissingField("model"))?;
    let max_tokens = obj
        .get("max_tokens")
        .or_else(|| obj.get("max_completion_tokens"))
        .and_then(Value::as_u64)
        .ok_or(TranslateError::MissingField("max_tokens"))?;
    let messages = obj
        .get("messages")
        .and_then(Value::as_array)
        .ok_or(TranslateError::MissingField("messages"))?;
    warn_unsupported(obj);
    let translated = translate_messages(messages)?;
    let mut out = json!({
        "model": model,
        "max_tokens": max_tokens,
        "stream": true,
        "messages": translated,
    });
    let system = collect_system(messages);
    if !system.is_empty() {
        out["system"] = Value::String(system);
    }
    apply_sampling(obj, &mut out);
    if let Some(stop) = obj.get("stop") {
        match stop {
            Value::String(s) if !s.is_empty() => out["stop_sequences"] = json!([s]),
            Value::Array(a) if !a.is_empty() => {
                out["stop_sequences"] = Value::Array(a.clone());
            }
            _ => {}
        }
    }
    Ok(out)
}
fn warn_unsupported(obj: &serde_json::Map<String, Value>) {
    for key in UNSUPPORTED_KEYS {
        if obj.contains_key(*key) {
            tracing::warn!("claude: {key} has no equivalent, omitted");
        }
    }
}
/// temperature/top_p 同名直映；二者并存时 top_p 优先并告警。
fn apply_sampling(obj: &serde_json::Map<String, Value>, out: &mut Value) {
    match obj.get("top_p") {
        Some(top_p) => {
            if obj.contains_key("temperature") {
                tracing::warn!("claude: top_p and temperature both set, top_p wins");
            }
            out["top_p"] = top_p.clone();
        }
        None => {
            if let Some(t) = obj.get("temperature") {
                out["temperature"] = t.clone();
            }
        }
    }
}
/// 抽取 system 消息文本按序合并；非文本内容块告警忽略。
fn collect_system(messages: &[Value]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for msg in messages {
        if msg.get("role").and_then(Value::as_str) == Some("system") {
            collect_text(msg.get("content"), &mut parts);
        }
    }
    parts.join("\n")
}
fn collect_text(content: Option<&Value>, out: &mut Vec<String>) {
    match content {
        Some(Value::String(s)) => out.push(s.clone()),
        Some(Value::Array(parts)) => {
            for part in parts {
                match part.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(t) = part.get("text").and_then(Value::as_str) {
                            out.push(t.to_string());
                        }
                    }
                    Some(other) => tracing::warn!("claude: non-text content block {other} ignored"),
                    None => tracing::warn!("claude: content block without type ignored"),
                }
            }
        }
        _ => tracing::warn!("claude: non-text message content ignored"),
    }
}
/// 过滤 system、拒绝 tool 相关角色/字段，其余归一为 Claude 消息。
fn translate_messages(messages: &[Value]) -> Result<Vec<Value>, TranslateError> {
    let mut out: Vec<Value> = Vec::new();
    for msg in messages {
        let Some(role) = msg.get("role").and_then(Value::as_str) else {
            tracing::warn!("claude: message without role ignored");
            continue;
        };
        match role {
            "system" => continue,
            "user" | "assistant" => {
                if msg.get("tool_calls").is_some() || msg.get("function_call").is_some() {
                    return Err(TranslateError::ToolUnsupported("tool_calls/function_call"));
                }
                let Some(content) = translated_content(msg) else {
                    continue;
                };
                let mut translated = json!({ "role": role, "content": content });
                if let Some(name) = msg.get("name").and_then(Value::as_str) {
                    translated["name"] = Value::String(name.to_string());
                }
                out.push(translated);
            }
            "tool" => return Err(TranslateError::ToolUnsupported("tool role")),
            other => tracing::warn!("claude: unknown role {other} ignored"),
        }
    }
    if out.is_empty() {
        return Err(TranslateError::BadShape(
            "no user/assistant messages".into(),
        ));
    }
    Ok(out)
}
/// content → Claude 内容：string 直用，text block 数组过滤为 text 块；
/// 无文本块返回 None（消息整体丢弃并告警，不静默）。
fn translated_content(msg: &Value) -> Option<Value> {
    match msg.get("content") {
        Some(Value::String(s)) => Some(Value::String(s.clone())),
        Some(Value::Array(parts)) => {
            let mut text: Vec<Value> = Vec::new();
            for part in parts {
                match part.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(t) = part.get("text") {
                            text.push(json!({"type": "text", "text": t}));
                        }
                    }
                    Some(other) => tracing::warn!("claude: non-text content block {other} ignored"),
                    None => tracing::warn!("claude: content block without type ignored"),
                }
            }
            if text.is_empty() {
                tracing::warn!("claude: message has no text content, dropped");
                None
            } else {
                Some(Value::Array(text))
            }
        }
        _ => {
            tracing::warn!("claude: non-text message content, dropped");
            None
        }
    }
}
/// baseUrl 归一：trim 尾斜杠；以 /v1 结尾拼 /messages，否则拼 /v1/messages（§5.2）。
pub fn normalize_base_url(base: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{trimmed}/messages")
    } else {
        format!("{trimmed}/v1/messages")
    }
}

pub struct ClaudeUpstream {
    client: reqwest::Client,
}

impl ClaudeUpstream {
    /// 连接超时 10s，与 HttpUpstream 同口径。
    pub fn new() -> std::io::Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(concat!("llm-share-proxy/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))?;
        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl Upstream for ClaudeUpstream {
    async fn chat(&self, call: UpstreamCall) -> Result<SseByteStream, UpstreamFailure> {
        let body = translate_request(&call.body)
            .map_err(|e| UpstreamFailure::Broken(format!("translate: {e}")))?;
        let bytes = serde_json::to_vec(&body)
            .map_err(|e| UpstreamFailure::Broken(format!("encode body: {e}")))?;
        let resp = self
            .client
            .post(normalize_base_url(&call.base_url))
            .header("x-api-key", &call.api_key)
            .header("anthropic-version", "2023-06-01")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(bytes)
            .send()
            .await
            .map_err(|e| UpstreamFailure::Broken(format!("claude connect: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            let (err_type, message) = read_error_body(resp).await;
            tracing::warn!(
                status = status.as_u16(),
                error_type = %err_type,
                error_message = %message,
                "claude upstream rejected"
            );
            return Err(UpstreamFailure::Rejected(status.as_u16()));
        }
        if !is_event_stream(&resp) {
            return Err(UpstreamFailure::Broken(
                "claude returned non-SSE body".into(),
            ));
        }
        let inner = resp
            .bytes_stream()
            .map(|chunk| {
                chunk
                    .map(|b| b.to_vec())
                    .map_err(|e| UpstreamFailure::Broken(e.to_string()))
            })
            .boxed();
        Ok(Box::pin(response::ClaudeSseTranslator::new(inner)))
    }

    fn protocol_hint(&self) -> &'static str {
        "claude"
    }
}

/// 读取上游错误体，仅提取 type/message 供日志脱敏记录（禁记 key，§5.2）。
async fn read_error_body(resp: reqwest::Response) -> (String, String) {
    let text = resp.text().await.unwrap_or_default();
    let truncated = text.chars().take(400).collect::<String>();
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return ("unknown".into(), truncated);
    };
    let err_type = v
        .pointer("/error/type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let message = v
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or(&truncated)
        .to_string();
    (err_type, message)
}
fn is_event_stream(resp: &reqwest::Response) -> bool {
    resp.headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.contains("text/event-stream"))
}

#[cfg(test)]
mod tests;
