//! admin HTTP 测试：三端点语义 + Bearer 鉴权 + 422/404/400 错误路径。

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

async fn spawn(tag: &str) -> (SocketAddr, String, AgentConfig, Arc<CaptureAudit>) {
    let cfg = AgentConfig {
        data_dir: std::env::temp_dir()
            .join(format!("acp-admin-{tag}-{}", std::process::id()))
            .to_string_lossy()
            .into_owned(),
        ..AgentConfig::default()
    };
    let _ = std::fs::remove_dir_all(&cfg.data_dir);
    std::fs::create_dir_all(&cfg.data_dir).expect("tmp dir");
    let audit = Arc::new(CaptureAudit::new());
    let policy = Arc::new(StdRwLock::new(PolicyTable::new()));
    let service = ShareService::open(&cfg, policy, audit.clone()).expect("service");
    let token = AdminToken::issue(cfg.paths().admin_token()).expect("token file");
    let value = token.value.clone();
    let server = AdminServer::start(
        0,
        value.clone(),
        AdminDeps {
            service: Arc::new(service),
            link: LinkContext {
                peer: "PEER_ADMIN_TEST".to_owned(),
                addrs: vec!["/ip4/127.0.0.1/udp/4001/quic-v1".to_owned()],
            },
        },
    )
    .await
    .expect("admin server");
    (server.addr, value, cfg, audit)
}

async fn http(
    addr: SocketAddr,
    method: &str,
    target: &str,
    bearer: Option<&str>,
    body: Option<&str>,
) -> (u16, String) {
    let mut tcp = TcpStream::connect(addr).await.expect("connect");
    let auth = match bearer {
        Some(token) => format!("Authorization: Bearer {token}\r\n"),
        None => String::new(),
    };
    let body_bytes = body.unwrap_or("").as_bytes();
    let request = format!(
        "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n{auth}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
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

const CREATE_BODY: &str =
    r#"{"scope":"sandbox","ttl_secs":3600,"max_activations":2,"note":"nb","allow_mcp":["fs"]}"#;

#[tokio::test]
async fn create_then_list_then_delete_roundtrip() {
    let (addr, token, cfg, _audit) = spawn("roundtrip").await;
    let (status, body) = http(addr, "POST", "/shares", Some(&token), Some(CREATE_BODY)).await;
    assert_eq!(status, 200, "创建应成功: {body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
    let token_plain = parsed["token"].as_str().expect("token").to_owned();
    let share_id = parsed["share_id"].as_str().expect("share_id").to_owned();
    assert_eq!(token_plain.len(), 32);
    assert_eq!(parsed["peer"], "PEER_ADMIN_TEST");
    assert!(
        parsed["link"].as_str().expect("link").starts_with(
            "dsh-acp-share://v1?peer=PEER_ADMIN_TEST&addr=/ip4/127.0.0.1/udp/4001/quic-v1&token="
        ),
        "链接要素组装不符: {}",
        parsed["link"]
    );
    let raw_ledger = std::fs::read_to_string(cfg.paths().shares()).expect("ledger");
    assert!(!raw_ledger.contains(&token_plain), "台账不得含 token 原文");

    let (status, body) = http(addr, "GET", "/shares", Some(&token), None).await;
    assert_eq!(status, 200);
    assert!(!body.contains(&token_plain), "列表不得含 token 原文");
    assert!(
        !body.contains(&acp_common::token_sha256(&token_plain)),
        "列表不得含哈希"
    );
    assert!(body.contains(&share_id));
    assert!(body.contains("\"status\":\"active\""));

    let (status, body) = http(
        addr,
        "DELETE",
        &format!("/shares/{share_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, 200, "撤销应成功: {body}");
    assert!(body.contains("\"revoked\":true"));

    let (status, _) = http(
        addr,
        "DELETE",
        &format!("/shares/{share_id}"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, 200, "重复撤销幂等成功");
}

#[tokio::test]
async fn auth_rejects_missing_and_wrong_bearer() {
    let (addr, token, _cfg, _audit) = spawn("auth").await;
    let (status, _) = http(addr, "GET", "/shares", None, None).await;
    assert_eq!(status, 401, "无 Bearer 必须 401");
    let (status, _) = http(addr, "GET", "/shares", Some("wrong-token"), None).await;
    assert_eq!(status, 401, "错 Bearer 必须 401");
    let (status, _) = http(addr, "POST", "/shares", Some(&token), Some(CREATE_BODY)).await;
    assert_eq!(status, 200, "正确 Bearer 放行");
}

#[tokio::test]
async fn workspace_scope_without_dir_is_422() {
    let (addr, token, _cfg, _audit) = spawn("ws422").await;
    let body = r#"{"scope":"workspace","ttl_secs":60}"#;
    let (status, body) = http(addr, "POST", "/shares", Some(&token), Some(body)).await;
    assert_eq!(status, 422, "workspace 未配置必须 422: {body}");
    assert!(body.contains("workspace-unconfigured"));
}

#[tokio::test]
async fn unknown_share_and_invalid_input_are_explicit() {
    let (addr, token, _cfg, _audit) = spawn("errors").await;
    let (status, body) = http(
        addr,
        "DELETE",
        "/shares/00000000-0000-0000-0000-000000000000",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, 404);
    assert!(body.contains("unknown-share"));
    let (status, _) = http(addr, "POST", "/shares", Some(&token), Some("not json")).await;
    assert_eq!(status, 400);
    let (status, body) = http(
        addr,
        "POST",
        "/shares",
        Some(&token),
        Some(r#"{"scope":"sandbox","ttl_secs":0}"#),
    )
    .await;
    assert_eq!(status, 400, "ttl=0 必须拒绝: {body}");
    let (status, _) = http(addr, "GET", "/nope", Some(&token), None).await;
    assert_eq!(status, 404);
    let (status, body) = http(
        addr,
        "POST",
        "/shares",
        Some(&token),
        Some(r#"{"scope":"owner","ttl_secs":60}"#),
    )
    .await;
    assert_eq!(status, 400, "owner scope 不可经分享产生: {body}");
}

#[tokio::test]
async fn token_file_is_private_mode_0600() {
    let (_addr, _token, cfg, _audit) = spawn("perm").await;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(cfg.paths().admin_token())
            .expect("token file")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "admin token 文件必须 0600");
    }
}

#[test]
fn ready_line_matches_frozen_shape() {
    let line = super::admin::ready_line(8123, &PathBuf::from("/tmp/acp-data/acp-admin-token"));
    let parsed: serde_json::Value = serde_json::from_str(&line).expect("json line");
    assert_eq!(parsed["kind"], "ready");
    assert_eq!(parsed["admin"]["port"], 8123);
    assert_eq!(
        parsed["admin"]["token_file"],
        "/tmp/acp-data/acp-admin-token"
    );
}
