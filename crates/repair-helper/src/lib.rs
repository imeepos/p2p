use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
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
struct Response<'a> {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorObject<'a>>,
}

#[derive(Debug, Serialize)]
struct ErrorObject<'a> {
    code: i32,
    message: &'a str,
}

#[derive(Clone)]
pub struct Host {
    registry: ToolRegistry,
}

impl Host {
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }
    pub fn empty() -> Self {
        Self::new(ToolRegistry::new())
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
    async fn dispatch(&self, request: Request) -> Option<Response<'static>> {
        let id = request.id?;
        if request.jsonrpc != "2.0" {
            return Some(error(id, -32600, "Invalid Request"));
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
                    Err(_) => return Some(error(id, -32602, "Invalid params")),
                };
                match self.registry.call(input).await {
                    Ok(value) => Some(
                        json!({"content":[{"type":"text","text":value.text}],"isError":false,"truncated":value.truncated}),
                    ),
                    Err(message) => {
                        tracing::warn!(%message, "tool call failed");
                        return Some(error(id, -32602, "Tool execution failed"));
                    }
                }
            }
            _ => return Some(error(id, -32601, "Method not found")),
        };
        Some(Response {
            jsonrpc: "2.0",
            id,
            result,
            error: None,
        })
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

fn error(id: Value, code: i32, message: &'static str) -> Response<'static> {
    Response {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(ErrorObject { code, message }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{duplex, AsyncReadExt, BufReader};
    use tokio::time::{sleep, Duration};

    async fn run(input: &str) -> String {
        let (mut client, server) = duplex(4096);
        let (reader, writer) = tokio::io::split(server);
        let (_tx, rx) = watch::channel(false);
        let host = Host::empty();
        let task = tokio::spawn(host.serve(BufReader::new(reader), writer, rx));
        client.write_all(input.as_bytes()).await.unwrap();
        client.shutdown().await.unwrap();
        let mut output = String::new();
        let mut reader = BufReader::new(client);
        reader.read_to_string(&mut output).await.unwrap();
        let _ = task.await;
        output
    }

    #[tokio::test]
    async fn initialize_matrix_and_unknown_method() {
        for version in SUPPORTED_VERSIONS {
            assert_eq!(Host::negotiate(Some(version)), version);
        }
        let output = run(r#"{"jsonrpc":"2.0","id":2,"method":"nope"}
"#)
        .await;
        assert!(output.contains("-32601"));
    }

    #[tokio::test]
    async fn list_is_empty_and_notification_has_no_reply() {
        let output = run("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n").await;
        assert!(output.contains("\"tools\":[]"));
        assert_eq!(output.matches("jsonrpc").count(), 1);
    }

    #[tokio::test]
    async fn shutdown_drains_in_flight_request() {
        struct Slow;
        #[async_trait]
        impl Tool for Slow {
            fn name(&self) -> &str {
                "slow"
            }
            async fn call(&self, _args: Value) -> Result<ToolResult, String> {
                sleep(Duration::from_millis(20)).await;
                Ok(ToolResult {
                    text: "done".into(),
                    truncated: false,
                })
            }
        }
        let mut registry = ToolRegistry::new();
        registry.register(Slow);
        let host = Host::new(registry);
        let (mut client, server) = duplex(4096);
        let (reader, writer) = tokio::io::split(server);
        let (tx, rx) = watch::channel(false);
        let task = tokio::spawn(host.serve(BufReader::new(reader), writer, rx));
        client.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/call\",\"params\":{\"name\":\"slow\"}}\n").await.unwrap();
        tx.send(true).unwrap();
        let mut output = String::new();
        BufReader::new(&mut client)
            .read_to_string(&mut output)
            .await
            .unwrap();
        assert!(output.contains("\"id\":7") && output.contains("done"));
        assert!(task.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn concurrent_calls_keep_ids() {
        struct Slow;
        #[async_trait]
        impl Tool for Slow {
            fn name(&self) -> &str {
                "slow"
            }
            async fn call(&self, args: Value) -> Result<ToolResult, String> {
                sleep(Duration::from_millis(args.as_u64().unwrap_or(1))).await;
                Ok(ToolResult {
                    text: args.to_string(),
                    truncated: false,
                })
            }
        }
        let mut registry = ToolRegistry::new();
        registry.register(Slow);
        let host = Host::new(registry);
        let (mut client, server) = duplex(4096);
        let (reader, writer) = tokio::io::split(server);
        let (_tx, rx) = watch::channel(false);
        let task = tokio::spawn(host.serve(BufReader::new(reader), writer, rx));
        client.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"slow\",\"arguments\":20}}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"slow\",\"arguments\":1}}\n").await.unwrap();
        client.shutdown().await.unwrap();
        let mut out = String::new();
        BufReader::new(client)
            .read_to_string(&mut out)
            .await
            .unwrap();
        assert!(out.contains("\"id\":1") && out.contains("\"id\":2"));
        let _ = task.await;
    }
}
