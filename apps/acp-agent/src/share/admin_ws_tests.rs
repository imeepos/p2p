//! admin 工作区管理端点测试：POST/DELETE /workspaces 语义、持久化与错误路径。

use std::net::SocketAddr;
use std::sync::{Arc, RwLock as StdRwLock};

use acp_common::policy::PolicyTable;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::admin::{AdminDeps, AdminServer, AdminToken};
use super::{LinkContext, ShareService};
use crate::audit::CaptureAudit;
use crate::config::AgentConfig;
use crate::workspaces::WorkspaceStore;

async fn spawn(tag: &str) -> (SocketAddr, String, AgentConfig) {
    let cfg = AgentConfig {
        data_dir: std::env::temp_dir()
            .join(format!("acp-admin-ws-{tag}-{}", std::process::id()))
            .to_string_lossy()
            .into_owned(),
        ..AgentConfig::default()
    };
    let _ = std::fs::remove_dir_all(&cfg.data_dir);
    std::fs::create_dir_all(&cfg.data_dir).expect("tmp dir");
    let audit = Arc::new(CaptureAudit::new());
    let policy = Arc::new(StdRwLock::new(PolicyTable::new()));
    let store =
        Arc::new(WorkspaceStore::open(&[], None, cfg.paths().workspaces()).expect("ws store"));
    let service = ShareService::open(&cfg, store.clone(), policy, audit).expect("service");
    let token = AdminToken::issue(cfg.paths().admin_token()).expect("token file");
    let value = token.value.clone();
    let server = AdminServer::start(
        0,
        value.clone(),
        AdminDeps {
            service: Arc::new(service),
            link: LinkContext {
                peer: "PEER_WS_TEST".to_owned(),
                addrs: vec!["/ip4/127.0.0.1/udp/4001/quic-v1".to_owned()],
            },
            workspaces: store,
        },
    )
    .await
    .expect("admin server");
    (server.addr, value, cfg)
}

async fn http(
    addr: SocketAddr,
    method: &str,
    target: &str,
    bearer: &str,
    body: Option<&str>,
) -> (u16, String) {
    let mut tcp = TcpStream::connect(addr).await.expect("connect");
    let body_bytes = body.unwrap_or("").as_bytes();
    let request = format!(
        "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {bearer}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body_bytes.len(),
        body.unwrap_or(""),
    );
    tcp.write_all(request.as_bytes()).await.expect("write");
    tcp.shutdown().await.expect("shutdown");
    let mut raw = Vec::new();
    tcp.read_to_end(&mut raw).await.expect("read");
    let text = String::from_utf8_lossy(&raw).into_owned();
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_owned();
    (status, body)
}

#[tokio::test]
async fn post_workspace_persists_and_lists() {
    let (addr, token, cfg) = spawn("post").await;
    let body = r#"{"id":"docs","name":"Docs","dir":"/tmp"}"#;
    let (status, body) = http(addr, "POST", "/workspaces", &token, Some(body)).await;
    assert_eq!(status, 200, "新增应成功: {body}");
    let (status, body) = http(addr, "GET", "/workspaces", &token, None).await;
    assert_eq!(status, 200);
    assert!(body.contains("\"docs\""), "清单应含新行: {body}");
    let reopened = WorkspaceStore::open(&[], None, cfg.paths().workspaces()).expect("reopen");
    assert!(
        reopened.rows().iter().any(|w| w.id == "docs"),
        "重启后仍应存在（持久化生效）"
    );
}

#[tokio::test]
async fn post_workspace_error_paths() {
    let (addr, token, _cfg) = spawn("errors").await;
    let (status, _) = http(addr, "POST", "/workspaces", &token, Some("not-json")).await;
    assert_eq!(status, 400, "坏 JSON 应 400");
    let dup = r#"{"id":"a","name":"A","dir":"/tmp"}"#;
    let (status, _) = http(addr, "POST", "/workspaces", &token, Some(dup)).await;
    assert_eq!(status, 200);
    let (status, body) = http(addr, "POST", "/workspaces", &token, Some(dup)).await;
    assert_eq!(status, 409, "重复 id 应 409: {body}");
    let rel = r#"{"id":"b","name":"B","dir":"rel/path"}"#;
    let (status, body) = http(addr, "POST", "/workspaces", &token, Some(rel)).await;
    assert_eq!(status, 422, "相对目录应 422: {body}");
    assert!(body.contains("invalid-dir"));
}

#[tokio::test]
async fn delete_workspace_roundtrip_and_unknown() {
    let (addr, token, _cfg) = spawn("delete").await;
    let body = r#"{"id":"tmp","name":"Tmp","dir":"/tmp"}"#;
    let (status, _) = http(addr, "POST", "/workspaces", &token, Some(body)).await;
    assert_eq!(status, 200);
    let (status, body) = http(addr, "DELETE", "/workspaces/tmp", &token, None).await;
    assert_eq!(status, 200, "删除应成功: {body}");
    assert!(body.contains("\"deleted\":true") || body.contains("\"deleted\": true"));
    let (status, _) = http(addr, "DELETE", "/workspaces/tmp", &token, None).await;
    assert_eq!(status, 404, "重复删除应 404");
}
