//! 流身份归属回环测试（底座 peer 随流下传迁移，ISSUE 2026-09-05 底座契约缺口）：
//! 双操作者并发各开一条流，各归属各的 PeerId 互不串扰（旧在线集绕行在多 peer
//! 在线时歧义误拒）；未知 peer 拒绝且审计记流上真实身份；裸流入口（无身份
//! 上下文）握手前即 fail-closed。

mod common;

use std::sync::Arc;
use std::time::Duration;

use acp_agent::{AcpHandler, AuditEvent, CaptureAudit, SessionDeps};
use acp_common::{parse_server_hello, Scope, ServerHello};
use common::{
    build_client, build_server, connect_and_stream, handshake_client, read_line, seed_quic,
    test_config, write_policy, PROTO,
};
use p2p::ProtocolId;
use p2p_protocol::{dispatch_inbound, open_with_protocol, HandlerRegistry};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_operators_attribute_to_own_stream_identity() {
    let granted = build_client("attr-a").await;
    let stranger = build_client("attr-b").await;
    let cfg = test_config("attr");
    // 策略表只授 A：B 不在表即「未知 peer」。旧在线集绕行下 A/B 同时在线即歧义
    // fail-closed（A 被误拒）；流身份直采后 A/B 各归属各流，授权判定互不串扰。
    write_policy(&cfg, Some((&granted.local_peer_id(), Scope::Sandbox)));
    let (server, audit) = build_server(&cfg).await;
    let server_peer = server.local_peer_id();
    seed_quic(&server, server_peer, &granted);
    seed_quic(&server, server_peer, &stranger);

    let mut stream_a = connect_and_stream(&granted, server_peer).await;
    let mut stream_b = connect_and_stream(&stranger, server_peer).await;

    let reply_a = handshake_client(&mut stream_a).await;
    let reply_b = handshake_client(&mut stream_b).await;
    assert!(
        matches!(reply_a, ServerHello::Ready { .. }),
        "granted operator must pass via own stream identity, got {reply_a:?}"
    );
    assert_eq!(
        reply_b,
        ServerHello::Denied {
            denied: "peer-not-allowed".to_owned(),
        },
        "unknown peer must be denied on the wire"
    );

    let granted_id = granted.local_peer_id().to_string();
    let stranger_id = stranger.local_peer_id().to_string();
    wait_audit(
        &audit,
        |ev| matches!(ev, AuditEvent::ConnEstablished { peer, .. } if *peer == granted_id),
    )
    .await;
    wait_audit(&audit, |ev| {
        matches!(ev, AuditEvent::ConnDenied { peer, code } if *peer == stranger_id && code == "peer-not-allowed")
    })
    .await;

    drop(stream_a);
    drop(stream_b);
    server.shutdown();
    granted.shutdown();
    stranger.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bare_stream_without_identity_fails_closed() {
    // 裸流入口（dispatch_inbound，无身份上下文）：握手前即拒绝并审计 unknown，
    // 证明旧签名路径不可能再产出归属（fail-closed，行为不回退）。
    let cfg = test_config("attr-bare");
    write_policy(&cfg, None);
    let audit = Arc::new(CaptureAudit::new());
    let deps = SessionDeps::assemble(cfg, audit.clone()).expect("deps");
    let mut registry = HandlerRegistry::default();
    registry.register(Arc::new(AcpHandler::new(deps).expect("handler")));

    let (client, server) = tokio::io::duplex(4096);
    let server_task = tokio::spawn(async move {
        let _ = dispatch_inbound(Box::new(server), &registry).await;
    });
    let mut stream = open_with_protocol(
        Box::new(client),
        &ProtocolId::new(PROTO).expect("valid protocol id"),
    )
    .await
    .expect("protocol handshake");

    let reply = read_line(&mut stream)
        .await
        .expect("denied frame on bare stream");
    let hello = parse_server_hello(reply.trim_end()).expect("parse server hello");
    assert_eq!(
        hello,
        ServerHello::Denied {
            denied: "peer-not-allowed".to_owned(),
        },
        "bare stream must be denied before handshake"
    );
    assert!(
        audit.contains(|ev| matches!(ev, AuditEvent::ConnDenied { peer, code }
            if peer == "unknown" && code == "peer-not-allowed")),
        "bare-stream denial must be audited: {:?}",
        audit.snapshot(),
    );
    server_task.await.expect("server task");
}

async fn wait_audit(audit: &Arc<CaptureAudit>, pred: impl Fn(&AuditEvent) -> bool + Copy) {
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
