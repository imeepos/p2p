//! ACP4 续连回环集成测试（设计 §5）主链路：断流窗口内不杀子进程、session/update
//! 入环形缓存、携票据重连后 initialize 过桥即补放（宣告行 + 原序缓存行）、
//! outstanding 权限立即 reject-once；错误票据拒绝并审计。

mod common;

use std::time::Duration;

use acp_agent::AuditEvent;
use acp_common::{AskRoute, Scope, ServerHello};
use common::{
    handshake_client, handshake_client_reattach, open_stream, permission_request, read_line,
    send_line, shutdown, test_config, test_grant_full, Rig,
};
use serde_json::Value;
use uuid::Uuid;

async fn rig_full(tag: &str, tweak: impl FnOnce(&mut acp_agent::AgentConfig)) -> Rig {
    let client = common::build_client(tag).await;
    let mut cfg = test_config(tag);
    tweak(&mut cfg);
    common::write_policy_full(
        &cfg,
        Some((
            &client.local_peer_id(),
            test_grant_full(Scope::Sandbox, Vec::new(), AskRoute::RemoteGui),
        )),
    );
    let (server, audit) = common::build_server(&cfg).await;
    let server_peer = server.local_peer_id();
    common::seed_quic(&server, server_peer, &client);
    Rig {
        server,
        audit,
        server_peer,
        client,
    }
}

fn update_seq(line: &str) -> u64 {
    let root: Value = serde_json::from_str(line.trim_end()).expect("json update");
    assert_eq!(
        root["method"], "session/update",
        "expected update, got: {line}"
    );
    root["params"]["seq"].as_u64().expect("seq")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reattach_full_chain_replays_cached_updates_in_order() {
    let rig = rig_full("re-full", |cfg| {
        cfg.reattach_window_secs = 6;
        cfg.command = vec![
            common::STUB.to_owned(),
            "--emit-updates".to_owned(),
            "0".to_owned(),
            "25".to_owned(),
            "--session".to_owned(),
            "s1".to_owned(),
        ];
    })
    .await;
    let mut first = open_stream(&rig).await;
    let ServerHello::Ready { ready } = handshake_client(&mut first).await else {
        panic!("first handshake must be ready")
    };
    let ticket = ready
        .ticket
        .clone()
        .expect("fresh connection must issue a reattach ticket");

    // 首连活性：两条实时 update，seq 单调
    let live1 = read_line(&mut first).await.expect("live update 1");
    assert_eq!(update_seq(&live1), 1);
    let live2 = read_line(&mut first).await.expect("live update 2");
    assert_eq!(update_seq(&live2), 2);

    // 注入 execute 权限请求（经回声桩等价子进程自发），客户端不批准即断流
    send_line(&mut first, &permission_request(77, "execute")).await;
    let forwarded = read_line(&mut first).await.expect("forwarded request");
    assert!(forwarded.contains("request_permission"), "{forwarded}");
    drop(first);
    // 条件等待替代盲睡：桥感知断流（ClientGone 审计）后再蓄窗口缓存
    let gone = tokio::time::Instant::now() + Duration::from_secs(5);
    while !rig
        .audit
        .contains(|ev| matches!(ev, AuditEvent::ClientGone { .. }))
    {
        assert!(
            tokio::time::Instant::now() < gone,
            "client gone not observed: {:?}",
            rig.audit.snapshot(),
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // 窗口内：缓存积累窗口期 update（并行负载下条数会抖，核心断言在顺序与衔接）
    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut second = open_stream(&rig).await;
    let hello =
        handshake_client_reattach(&mut second, Uuid::parse_str(&ticket).expect("uuid")).await;
    let ServerHello::Ready { .. } = hello else {
        panic!("reattach must be accepted, got {hello:?}")
    };

    // initialize 过桥后：先宣告行，后补放缓存，序号严格递增
    send_line(
        &mut second,
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}",
    )
    .await;
    let announce = read_line(&mut second).await.expect("announcement");
    let ann: Value = serde_json::from_str(announce.trim_end()).expect("json");
    assert_eq!(ann["method"], "dsh/bridge/reattach", "{announce}");
    let replayed = ann["params"]["replayed"].as_u64().expect("count");
    assert!(replayed >= 1, "window must have cached updates: {announce}");

    let mut last = 2_u64;
    for _ in 0..replayed {
        let line = read_line(&mut second).await.expect("replayed update");
        let seq = update_seq(&line);
        assert!(
            seq > last,
            "replay must be strictly ordered: {seq} after {last}"
        );
        last = seq;
    }
    // 补放完成后恢复实时透传（initialize 回声等透传行先行到达，跳过）
    let live = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let line = read_line(&mut second).await.expect("line");
            if line.contains("\"session/update\"") {
                break line;
            }
        }
    })
    .await
    .expect("live updates resume");
    assert!(
        update_seq(&live) > last,
        "live seq must continue after replay"
    );

    // outstanding 已在断流瞬间 reject-once（无人值守 = 拒绝）
    assert!(
        rig.audit.contains(|ev| matches!(
            ev,
            AuditEvent::PermissionActed { action, .. } if action == "unanswered-rejected"
        )),
        "detach must reject outstanding once: {:?}",
        rig.audit.snapshot(),
    );
    assert!(
        rig.audit.contains(|ev| matches!(
            ev,
            AuditEvent::ReattachAccepted { detail, .. } if detail.starts_with("replayed=")
        )),
        "reattach must be audited: {:?}",
        rig.audit.snapshot(),
    );

    drop(second);
    shutdown(&rig);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn wrong_ticket_is_denied_and_audited() {
    let rig = rig_full("re-wrong", |cfg| {
        cfg.reattach_window_secs = 6;
    })
    .await;
    let mut first = open_stream(&rig).await;
    handshake_client(&mut first).await;
    drop(first);
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut second = open_stream(&rig).await;
    let hello = handshake_client_reattach(&mut second, Uuid::new_v4()).await;
    assert_eq!(
        hello,
        ServerHello::Denied {
            denied: "reattach-ticket-invalid".to_owned(),
        },
        "unknown ticket must be denied",
    );
    assert!(
        rig.audit
            .contains(|ev| matches!(ev, AuditEvent::ReattachDenied { .. })),
        "ticket denial must be audited: {:?}",
        rig.audit.snapshot(),
    );
    drop(second);
    shutdown(&rig);
}
