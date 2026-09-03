pub mod audit;
pub mod cap;
pub mod enforce;
pub mod jail;
pub mod tools;

use async_trait::async_trait;
use audit::{AuditEvent, AuditSink};
use enforce::Enforcement;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{watch, Mutex};

pub const SUPPORTED_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallInput {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolResult {
    pub text: String,
    pub truncated: bool,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str {
        ""
    }
    async fn call(&self, arguments: Value) -> Result<ToolResult, String>;
}

#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: Arc<Vec<Arc<dyn Tool>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        Arc::make_mut(&mut self.tools).push(Arc::new(tool));
    }
    fn list(&self) -> Value {
        let tools = self
            .tools
            .iter()
            .map(|tool| json!({"name": tool.name(), "description": tool.description()}))
            .collect::<Vec<_>>();
        json!({"tools": tools})
    }
    async fn call(&self, input: ToolCallInput) -> Result<ToolResult, String> {
        match self.tools.iter().find(|tool| tool.name() == input.name) {
            Some(tool) => tool.call(input.arguments).await,
            None => Err(format!("unknown tool: {}", input.name)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Request {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct Response {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorObject>,
}

#[derive(Debug, Serialize)]
struct ErrorObject {
    code: i32,
    message: String,
}

#[derive(Clone)]
pub struct Host {
    registry: ToolRegistry,
    enforcement: Option<Enforcement>,
    audit: AuditSink,
}

impl Host {
    pub fn new(registry: ToolRegistry) -> Self {
        Self {
            registry,
            enforcement: None,
            audit: AuditSink::default(),
        }
    }
    pub fn empty() -> Self {
        Self::new(ToolRegistry::new())
    }
    /// 生产装配：tools/call 每次经执法分级后放行/拒绝，全程审计。
    pub fn guarded(registry: ToolRegistry, enforcement: Enforcement, audit: AuditSink) -> Self {
        Self {
            registry,
            enforcement: Some(enforcement),
            audit,
        }
    }
    pub fn negotiate(requested: Option<&str>) -> &'static str {
        match requested {
            Some(version) => SUPPORTED_VERSIONS
                .iter()
                .find(|candidate| **candidate == version)
                .copied()
                .unwrap_or(SUPPORTED_VERSIONS[0]),
            None => SUPPORTED_VERSIONS[0],
        }
    }
    async fn dispatch(&self, request: Request) -> Option<Response> {
        let id = request.id?;
        if request.jsonrpc != "2.0" {
            return Some(error(id, -32600, "Invalid Request".to_string()));
        }
        let result = match request.method.as_str() {
            "initialize" => {
                let requested = request
                    .params
                    .get("protocolVersion")
                    .and_then(Value::as_str);
                Some(
                    json!({"protocolVersion": Self::negotiate(requested), "capabilities": {"tools": {}}, "serverInfo": {"name": "repair-helper", "version": env!("CARGO_PKG_VERSION")}}),
                )
            }
            "ping" => Some(json!({})),
            "tools/list" => Some(self.registry.list()),
            "tools/call" => {
                let input = match serde_json::from_value::<ToolCallInput>(request.params) {
                    Ok(value) => value,
                    Err(_) => return Some(error(id, -32602, "Invalid params".to_string())),
                };
                Some(self.call_tool(input).await)
            }
            _ => return Some(error(id, -32601, "Method not found".to_string())),
        };
        Some(Response {
            jsonrpc: "2.0",
            id,
            result,
            error: None,
        })
    }

    /// tools/call 处理：执法分级在先（guard 宿主），放行后执行，全程审计。
    async fn call_tool(&self, input: ToolCallInput) -> Value {
        let started = Instant::now();
        let tool = input.name.clone();
        let params = cap::head(
            &serde_json::to_string(&input.arguments).unwrap_or_default(),
            256,
        );
        let risk = enforce::classify(&tool, &input.arguments);
        let risk_name = enforce::risk_name(risk);
        let duration = || started.elapsed().as_millis() as u64;
        if let Some(enforcement) = &self.enforcement {
            if let Err(reason) = enforcement.evaluate(&tool, &input.arguments) {
                tracing::warn!(%tool, %reason, "tool call denied by enforcement");
                let text = format!("tool error: {reason}");
                self.audit.push(AuditEvent::new(
                    tool,
                    params,
                    risk_name,
                    "denied",
                    reason,
                    duration(),
                ));
                return tool_result_json(&text, true);
            }
        }
        match self.registry.call(input).await {
            Ok(result) => {
                let gated = cap::apply_output_gate(result);
                let summary = cap::head(&gated.text, 120);
                self.audit.push(AuditEvent::new(
                    tool,
                    params,
                    risk_name,
                    "ok",
                    summary,
                    duration(),
                ));
                json!({
                    "content": [{"type": "text", "text": gated.text}],
                    "isError": false,
                    "truncated": gated.truncated
                })
            }
            Err(message) => {
                tracing::warn!(%message, "tool call failed");
                let text = format!("tool error: {message}");
                self.audit.push(AuditEvent::new(
                    tool,
                    params,
                    risk_name,
                    "error",
                    message,
                    duration(),
                ));
                tool_result_json(&text, true)
            }
        }
    }

    pub async fn serve<R, W>(
        self,
        reader: R,
        writer: W,
        mut shutdown: watch::Receiver<bool>,
    ) -> std::io::Result<()>
    where
        R: AsyncBufRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let writer = Arc::new(Mutex::new(writer));
        let mut lines = reader.lines();
        let mut tasks = Vec::new();
        loop {
            tokio::select! {
                changed = shutdown.changed() => { if changed.is_ok() && *shutdown.borrow() { break; } }
                line = lines.next_line() => {
                    let Some(line) = line? else { break };
                    if line.trim().is_empty() { continue; }
                    let host = self.clone(); let output = writer.clone();
                    tasks.push(tokio::spawn(async move {
                        let request = match serde_json::from_str::<Request>(&line) { Ok(request) => request, Err(_) => { tracing::warn!("invalid JSON-RPC request"); return Ok::<(), std::io::Error>(()); } };
                        if request.method == "notifications/initialized" && request.id.is_none() { return Ok(()); }
                        if let Some(response) = host.dispatch(request).await {
                            let bytes = serde_json::to_vec(&response).map_err(std::io::Error::other)?;
                            let mut output = output.lock().await; output.write_all(&bytes).await?; output.write_all(b"\n").await?; output.flush().await?;
                        }
                        Ok(())
                    }));
                }
            }
        }
        for task in tasks {
            task.await.map_err(std::io::Error::other)??;
        }
        Ok(())
    }
}

fn error(id: Value, code: i32, message: String) -> Response {
    Response {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(ErrorObject { code, message }),
    }
}

/// 工具调用结果统一封装：文本 + isError（带原因）+ truncated 门禁标记。
fn tool_result_json(text: &str, is_error: bool) -> Value {
    json!({
        "content": [{"type": "text", "text": text}],
        "isError": is_error,
        "truncated": false
    })
}

#[cfg(test)]
mod guarded_tests;
#[cfg(test)]
mod tests;
