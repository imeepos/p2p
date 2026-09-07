//! admin HTTP 浏览器跨源行为回归（2026-09-07 实证）：WebView fetch 对
//! Authorization 请求先发无凭据 OPTIONS 预检，旧实现按 401 挡掉导致
//! GUI「生成链接」全挂——裸 TCP 客户端测试覆盖不到这一层，必须用带
//! Origin 的客户端模拟浏览器行为矩阵：预检放行/实响应 ACAO/白名单外拒绝。

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock as StdRwLock};

use acp_common::policy::PolicyTable;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::admin::{AdminDeps, AdminServer, AdminToken};
use super::{LinkContext, ShareService};
use crate::audit::CaptureAudit;
use crate::config::AgentConfig;

const CREATE_BODY: &str = r#"{"scope":"sandbox","ttl_secs":3600,"max_activations":1}"#;

async fn spawn(tag: &str) -> (SocketAddr, String) {
    let cfg = AgentConfig {
        data_dir: std::env::temp_dir()
            .join(format!("acp-cors-{tag}-{}", std::process::id()))
            .to_string_lossy()
            .into_owned(),
        ..AgentConfig::default()
    };
    let _ = std::fs::remove_dir_all(&cfg.data_dir);
    std::fs::create_dir_all(&cfg.data_dir).expect("tmp dir");
    let audit = Arc::new(CaptureAudit::new());
    let policy = Arc::new(StdRwLock::new(PolicyTable::new()));
    let service = ShareService::open(&cfg, policy, audit).expect("service");
    let token = AdminToken::issue(cfg.paths().admin_token()).expect("token file");
    let value = token.value.clone();
    let server = AdminServer::start(
        0,
        value.clone(),
        AdminDeps {
            service: Arc::new(service),
            link: LinkContext {
                peer: "PEER_CORS_TEST".to_owned(),
                addrs: vec!["/ip4/127.0.0.1/udp/4001/quic-v1".to_owned()],
            },
        },
    )
    .await
    .expect("admin server");
    (server.addr, value)
}

/// 带 Origin 头的请求（模拟浏览器跨源 fetch）：返回 (状态, 响应头段, 响应体)。
async fn http_origin(
    addr: SocketAddr,
    method: &str,
    target: &str,
    bearer: Option<&str>,
    body: Option<&str>,
    origin: &str,
) -> (u16, String, String) {
    let mut tcp = TcpStream::connect(addr).await.expect("connect");
    let auth = match bearer {
        Some(token) => format!("Authorization: Bearer {token}\r\n"),
        None => String::new(),
    };
    let body_bytes = body.unwrap_or("").as_bytes();
    let request = format!(
        "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: {origin}\r\n{auth}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        body_bytes.len(),
        body.unwrap_or(""),
    );
    tcp.write_all(request.as_bytes()).await.expect("write");
    tcp.shutdown().await.expect("shutdown");
    let mut raw = Vec::new();
    tcp.read_to_end(&mut raw).await.expect("read");
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, resp_body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let status: u16 = head
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (status, head.to_owned(), resp_body.to_owned())
}

#[tokio::test]
async fn browser_preflight_needs_no_auth_and_gets_cors_headers() {
    let (addr, _token) = spawn("preflight").await;
    let (status, head, _body) =
        http_origin(addr, "OPTIONS", "/shares", None, None, "tauri://localhost").await;
    assert_eq!(
        status, 204,
        "预检必须 204：无凭据预检不得被 Bearer 校验挡掉"
    );
    assert!(
        head.contains("Access-Control-Allow-Origin: tauri://localhost"),
        "{head}"
    );
    assert!(head.contains("Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS"));
    assert!(head.contains("Access-Control-Allow-Headers: Authorization, Content-Type"));
}

#[tokio::test]
async fn app_origin_actual_response_carries_allow_origin() {
    let (addr, token) = spawn("acao").await;
    let (status, head, _body) = http_origin(
        addr,
        "POST",
        "/shares",
        Some(&token),
        Some(CREATE_BODY),
        "http://tauri.localhost",
    )
    .await;
    assert_eq!(status, 200);
    assert!(
        head.contains("Access-Control-Allow-Origin: http://tauri.localhost"),
        "{head}"
    );
}

#[tokio::test]
async fn foreign_origin_gets_no_cors_headers() {
    let (addr, token) = spawn("evil").await;
    let (status, head, _body) = http_origin(
        addr,
        "POST",
        "/shares",
        Some(&token),
        Some(CREATE_BODY),
        "http://evil.example",
    )
    .await;
    assert_eq!(status, 200, "白名单外 origin 不改变服务端语义");
    assert!(!head.contains("Access-Control-Allow-Origin"), "{head}");
}

#[tokio::test]
async fn preflight_from_foreign_origin_is_403() {
    let (addr, _token) = spawn("preflight-evil").await;
    let (status, _head, _body) = http_origin(
        addr,
        "OPTIONS",
        "/shares",
        None,
        None,
        "http://evil.example",
    )
    .await;
    assert_eq!(status, 403, "白名单外 origin 的预检必须 403");
}

#[test]
fn ready_line_shape_still_frozen() {
    let line = super::admin::ready_line(8123, &PathBuf::from("/tmp/x/acp-admin-token"));
    let parsed: serde_json::Value = serde_json::from_str(&line).expect("json line");
    assert_eq!(parsed["admin"]["port"], 8123);
}
