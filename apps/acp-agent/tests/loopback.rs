//! 单机回环集成测试（卡 ACP2-E）：双 facade 节点 + acp-echo-stub 桩，覆盖
//! 未授权拒绝 / 授权握手 / 透传 roundtrip / 超上限拒绝 / 子进程退出断流 /
//! spawn 失败断流。审计断言走 CaptureAudit。

mod common;

use std::time::Duration;

use acp_agent::AuditEvent;
use acp_common::{Scope, ServerHello};
use common::STUB;
use common::{
    build_client, build_server, connect_and_stream, handshake_client, read_line, seed_quic,
    send_line, test_config, write_policy,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unauthorized_peer_gets_denied_and_audited() {
    let client = build_client("unauth").await;
    let cfg = test_config("unauth");
    write_policy(&cfg, None);
    let (server, audit) = build_server(&cfg).await;
    let server_peer = server.local_peer_id();
    seed_quic(&server, server_peer, &client);

    let mut stream = connect_and_stream(&client, server_peer).await;
    let reply = handshake_client(&mut stream).await;
    assert_eq!(
        reply,
        ServerHello::Denied {
            denied: "peer-not-allowed".to_owned(),
        },
        "unauthorized peer must receive observable denied frame",
    );
    assert!(
        audit.contains(|ev| matches!(
            ev,
            AuditEvent::ConnDenied { code, .. } if code == "peer-not-allowed"
        )),
        "denial must be audited: {:?}",
        audit.snapshot(),
    );

    server.shutdown();
    client.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authorized_handshake_replies_ready() {
    let client = build_client("ready").await;
    let cfg = test_config("ready");
    write_policy(&cfg, Some((&client.local_peer_id(), Scope::Sandbox)));
    let (server, audit) = build_server(&cfg).await;
    let server_peer = server.local_peer_id();
    seed_quic(&server, server_peer, &client);

    let mut stream = connect_and_stream(&client, server_peer).await;
    let reply = handshake_client(&mut stream).await;
    let ServerHello::Ready { ready } = reply else {
        panic!("authorized handshake must reply ready, got {reply:?}")
    };
    assert_eq!(ready.scope, Scope::Sandbox);
    assert_eq!(ready.agent, cfg.agent_name);
    assert_eq!(ready.bridge, "1");
    assert!(
        audit.contains(|ev| matches!(ev, AuditEvent::ConnEstablished { .. })),
        "established must be audited: {:?}",
        audit.snapshot(),
    );

    drop(stream);
    wait_audit(&audit, |ev| matches!(ev, AuditEvent::SubprocessExit { .. })).await;
    server.shutdown();
    client.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn passthrough_roundtrip_and_stderr_capture() {
    let client = build_client("pump").await;
    let mut cfg = test_config("pump");
    cfg.command = vec![STUB.to_owned(), "--say-stderr".to_owned()];
    write_policy(&cfg, Some((&client.local_peer_id(), Scope::Sandbox)));
    let (server, _audit) = build_server(&cfg).await;
    let server_peer = server.local_peer_id();
    seed_quic(&server, server_peer, &client);

    let mut stream = connect_and_stream(&client, server_peer).await;
    handshake_client(&mut stream).await;

    let request = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}";
    send_line(&mut stream, request).await;
    let echoed = read_line(&mut stream)
        .await
        .expect("echo line within wait")
        .trim_end()
        .to_owned();
    assert_eq!(echoed, request, "bridge must be byte-transparent");

    // 子进程 stderr 已被接管并落滚动日志（stub --say-stderr 启动行）
    let log_dir = cfg.log_dir();
    wait_log_contains(&log_dir, "acp-echo-stub: ready").await;

    drop(stream);
    server.shutdown();
    client.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn second_stream_over_per_peer_cap_is_denied() {
    let client = build_client("cap").await;
    let mut cfg = test_config("cap");
    cfg.max_connections = 1;
    write_policy(&cfg, Some((&client.local_peer_id(), Scope::Sandbox)));
    let (server, audit) = build_server(&cfg).await;
    let server_peer = server.local_peer_id();
    seed_quic(&server, server_peer, &client);

    let mut first = connect_and_stream(&client, server_peer).await;
    let ready = handshake_client(&mut first).await;
    assert!(
        matches!(ready, ServerHello::Ready { .. }),
        "first conn must pass"
    );

    let mut second = connect_and_stream(&client, server_peer).await;
    let denied = handshake_client(&mut second).await;
    assert_eq!(
        denied,
        ServerHello::Denied {
            denied: "conn-cap-reached".to_owned(),
        },
    );
    assert!(
        audit.contains(|ev| matches!(ev, AuditEvent::GateDenied { .. })),
        "gate denial must be audited: {:?}",
        audit.snapshot(),
    );

    drop(first);
    drop(second);
    server.shutdown();
    client.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn child_exit_closes_stream_and_audits() {
    let client = build_client("exit").await;
    let cfg = test_config("exit");
    write_policy(&cfg, Some((&client.local_peer_id(), Scope::Sandbox)));
    let (server, audit) = build_server(&cfg).await;
    let server_peer = server.local_peer_id();
    seed_quic(&server, server_peer, &client);

    let mut stream = connect_and_stream(&client, server_peer).await;
    handshake_client(&mut stream).await;
    send_line(&mut stream, "{\"stub_cmd\":\"acp-stub-exit\"}").await;

    let closed = tokio::time::timeout(Duration::from_secs(10), read_line(&mut stream)).await;
    assert!(
        closed.is_err() || closed.expect("no panic").is_none(),
        "bridge must close wire after subprocess exit"
    );
    wait_audit(&audit, |ev| matches!(ev, AuditEvent::SubprocessExit { .. })).await;

    server.shutdown();
    client.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_failure_denies_and_audits() {
    let client = build_client("spawn").await;
    let mut cfg = test_config("spawn");
    cfg.command = vec!["/nonexistent/acp-stub".to_owned()];
    write_policy(&cfg, Some((&client.local_peer_id(), Scope::Sandbox)));
    let (server, audit) = build_server(&cfg).await;
    let server_peer = server.local_peer_id();
    seed_quic(&server, server_peer, &client);

    let mut stream = connect_and_stream(&client, server_peer).await;
    let reply = handshake_client(&mut stream).await;
    assert_eq!(
        reply,
        ServerHello::Denied {
            denied: "subprocess-failed".to_owned(),
        },
        "spawn failure must be observable on the wire",
    );
    assert!(
        audit.contains(|ev| matches!(ev, AuditEvent::SpawnFailed { .. })),
        "spawn failure must be audited: {:?}",
        audit.snapshot(),
    );

    server.shutdown();
    client.shutdown();
}

async fn wait_audit(audit: &acp_agent::CaptureAudit, pred: impl Fn(&AuditEvent) -> bool + Copy) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if audit.contains(pred) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for audit event: {:?}",
            audit.snapshot(),
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn wait_log_contains(dir: &std::path::Path, needle: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(mut entries) = std::fs::read_dir(dir) {
            for entry in entries.by_ref().flatten() {
                if let Ok(body) = std::fs::read_to_string(entry.path()) {
                    if body.contains(needle) {
                        return;
                    }
                }
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {needle} under {}",
            dir.display(),
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
