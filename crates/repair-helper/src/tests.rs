use super::*;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration};

async fn run(input: &str) -> String {
    let (mut client, server) = duplex(4096);
    let (reader, writer) = tokio::io::split(server);
    let (_tx, rx) = watch::channel(false);
    let task = tokio::spawn(Host::empty().serve(BufReader::new(reader), writer, rx));
    client.write_all(input.as_bytes()).await.unwrap();
    client.shutdown().await.unwrap();
    let mut output = String::new();
    BufReader::new(client)
        .read_to_string(&mut output)
        .await
        .unwrap();
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
    let (release_tx, release_rx) = oneshot::channel();
    let gate = std::sync::Arc::new(tokio::sync::Mutex::new(Some(release_rx)));
    struct Slow(std::sync::Arc<tokio::sync::Mutex<Option<oneshot::Receiver<()>>>>);
    #[async_trait]
    impl Tool for Slow {
        fn name(&self) -> &str {
            "slow"
        }
        async fn call(&self, _args: Value) -> Result<ToolResult, String> {
            if let Some(receiver) = self.0.lock().await.take() {
                let _ = receiver.await;
            }
            Ok(ToolResult {
                text: "done".into(),
                truncated: false,
            })
        }
    }
    let mut registry = ToolRegistry::new();
    registry.register(Slow(gate));
    let (mut client, server) = duplex(4096);
    let (reader, writer) = tokio::io::split(server);
    let (tx, rx) = watch::channel(false);
    let task = tokio::spawn(Host::new(registry).serve(BufReader::new(reader), writer, rx));
    client.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/call\",\"params\":{\"name\":\"slow\"}}\n").await.unwrap();
    sleep(Duration::from_millis(50)).await;
    tx.send(true).unwrap();
    release_tx.send(()).unwrap();
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
    let (mut client, server) = duplex(4096);
    let (reader, writer) = tokio::io::split(server);
    let (_tx, rx) = watch::channel(false);
    let task = tokio::spawn(Host::new(registry).serve(BufReader::new(reader), writer, rx));
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
