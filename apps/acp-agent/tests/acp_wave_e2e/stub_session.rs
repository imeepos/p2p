//! 波次场景 ①-④（stub 模式，设计 §4.1/§6/§7）：未授权握手拒绝+审计、
//! 授权握手与 ndjson 透传 roundtrip、prompt→桩应答流、工具权限 ask→客户端
//! 批准放行→一次性 grant（桥不记忆许可，同 ask 必须再次透传）。

use acp_common::{AskRoute, PeerPolicy, Scope, ServerHello};
use serde_json::Value;

use acp_agent::AuditEvent;

use crate::common::{
    build_client, build_server, connect_and_stream, handshake_client, open_stream,
    permission_approve, permission_request, read_line, rig, seed_quic, send_line, shutdown,
    test_config, test_grant_full, write_policy,
};
use crate::line_within;

fn grant() -> PeerPolicy {
    test_grant_full(Scope::Sandbox, Vec::new(), AskRoute::RemoteGui)
}

/// ① 未授权 peer 连入：denied 帧可观察 + 审计（设计 §7 第 5 行 / §12-Q5）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn s1_unauthorized_handshake_denied_and_audited() {
    let client = build_client("wave-s1").await;
    let cfg = test_config("wave-s1");
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
        "unauthorized peer must get an observable denied frame",
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

/// ② 授权握手 ready（scope/agent/bridge/票据齐全）→ 之后纯 ndjson 透传 roundtrip。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn s2_authorized_handshake_then_ndjson_roundtrip() {
    let r = rig("wave-s2", grant(), |_| {}).await;
    let mut stream = open_stream(&r).await;
    let ServerHello::Ready { ready } = handshake_client(&mut stream).await else {
        panic!("authorized handshake must be ready");
    };
    assert_eq!(ready.scope, Scope::Sandbox);
    assert_eq!(ready.agent, "home-agent");
    assert_eq!(ready.bridge, "1");
    assert!(
        ready.ticket.is_some(),
        "fresh connection must carry a reattach ticket",
    );

    let request = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}";
    send_line(&mut stream, request).await;
    let echoed = read_line(&mut stream).await.expect("roundtrip line");
    assert_eq!(
        echoed.trim_end(),
        request,
        "bridge must be byte-transparent on the passthrough path",
    );
    shutdown(&r);
}

/// ③ prompt→桩应答流：prompt 行透传到桩并回声应答，session/update 流并行到达。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s3_prompt_streamed_through_stub_with_update_flow() {
    let r = rig("wave-s3", grant(), |cfg| {
        cfg.command = vec![
            crate::common::STUB.to_owned(),
            "--emit-updates".to_owned(),
            "0".to_owned(),
            "30".to_owned(),
        ];
    })
    .await;
    let mut stream = open_stream(&r).await;
    handshake_client(&mut stream).await;
    let prompt = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "session/prompt",
        "params": { "sessionId": "s1", "prompt": [{ "type": "text", "text": "hi" }] },
    })
    .to_string();
    send_line(&mut stream, &prompt).await;

    let mut prompt_answered = false;
    let mut updates = 0_u32;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(8);
    while (!prompt_answered || updates == 0) && tokio::time::Instant::now() < deadline {
        let Some(line) = line_within(&mut stream, 8).await else {
            break;
        };
        let Ok(root) = serde_json::from_str::<Value>(line.trim_end()) else {
            continue;
        };
        if root.get("id").and_then(Value::as_i64) == Some(3) {
            assert_eq!(root["method"], "session/prompt", "echo must match: {line}");
            prompt_answered = true;
        }
        if root["method"] == "session/update" {
            updates += 1;
        }
    }
    assert!(prompt_answered, "prompt must round-trip via stub echo");
    assert!(updates > 0, "update stream must flow while prompt in flight");
    shutdown(&r);
}

/// ④ 工具权限 ask→客户端批准放行→一次性 grant：execute ask 透传客户端，
/// 批准经桥写入子进程结算；同 ask 再次到达必须再次透传（桥不持久化许可）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn s4_ask_approved_by_client_and_grant_is_one_shot() {
    let r = rig("wave-s4", grant(), |_| {}).await;
    let mut stream = open_stream(&r).await;
    handshake_client(&mut stream).await;

    send_line(&mut stream, &permission_request(11, "execute")).await;
    let forwarded = read_line(&mut stream).await.expect("forwarded ask");
    assert!(forwarded.contains("request_permission"), "{forwarded}");
    send_line(&mut stream, &permission_approve(11, "allow-once")).await;
    let settled = read_line(&mut stream).await.expect("approval settlement");
    let res: Value = serde_json::from_str(settled.trim_end()).expect("json");
    assert_eq!(
        res["result"]["outcome"]["optionId"],
        "allow-once",
        "client approval must settle the ask: {settled}",
    );

    send_line(&mut stream, &permission_request(12, "execute")).await;
    let again = read_line(&mut stream).await.expect("second ask must forward");
    let root: Value = serde_json::from_str(again.trim_end()).expect("json");
    assert_eq!(
        root["id"], 12,
        "grant is one-shot: the same ask must be forwarded again, not auto-allowed: {again}",
    );

    let events = r.audit.snapshot();
    let forwards = events
        .iter()
        .filter(|ev| {
            matches!(ev, AuditEvent::PermissionActed { action, .. } if action == "forwarded")
        })
        .count();
    assert_eq!(forwards, 2, "both asks must be forwarded: {events:?}");
    assert!(
        !events.iter().any(|ev| matches!(
            ev,
            AuditEvent::PermissionActed { action, .. } if action == "auto-allowed"
        )),
        "execute kind must never auto-allow: {events:?}",
    );
    shutdown(&r);
}