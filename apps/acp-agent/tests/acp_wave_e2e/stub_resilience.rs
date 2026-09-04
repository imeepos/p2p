//! 波次场景 ⑤-⑦（stub 模式，设计 §5/§7）：断流窗口内同 PeerId 续连补放缓存
//! update（序与条数断言）、窗口过期走退出阶梯（桩退出被观测）、连接门禁超限
//! （total 与 per-peer 两级；每连接会话 ≤4 由子进程 ACP 侧执行，桥侧门禁是连接数）。

use std::time::Duration;

use acp_common::{AskRoute, PeerPolicy, Scope, ServerHello};
use serde_json::Value;
use uuid::Uuid;

use acp_agent::{reattach, AuditEvent};

use crate::common::{
    handshake_client, handshake_client_reattach, open_stream, permission_request, read_line, rig,
    send_line, shutdown, test_grant_full,
};
use crate::wait_audit;

fn grant() -> PeerPolicy {
    test_grant_full(Scope::Sandbox, Vec::new(), AskRoute::RemoteGui)
}

fn emitting_command() -> Vec<String> {
    vec![
        crate::common::STUB.to_owned(),
        "--emit-updates".to_owned(),
        "0".to_owned(),
        "25".to_owned(),
        "--session".to_owned(),
        "s1".to_owned(),
    ]
}

fn seq_of(line: &str) -> u64 {
    let root: Value = serde_json::from_str(line.trim_end()).expect("json update");
    assert_eq!(root["method"], "session/update", "expected update: {line}");
    root["params"]["seq"].as_u64().expect("seq")
}

/// ⑤ 客户端断流→窗口内同 PeerId 携票据 reattach→initialize 过桥后补放缓存
/// update：宣告条数 == 实际补放条数，会话名一致，seq 严格递增，实时透传接续；
/// 无人值守挂起的 ask 在断流瞬间 reject-once（设计 §5 安全侧）。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s5_reattach_replays_cached_updates_with_order_and_count() {
    let r = rig("wave-s5", grant(), |cfg| {
        // 生产默认 90s（设计 §12-Q1）；用例缩短窗口只为时长，语义同设计 §5
        cfg.reattach_window_secs = 6;
        cfg.command = emitting_command();
    })
    .await;
    let mut first = open_stream(&r).await;
    let ServerHello::Ready { ready } = handshake_client(&mut first).await else {
        panic!("first handshake must be ready");
    };
    let ticket = Uuid::parse_str(ready.ticket.expect("ticket").as_str()).expect("ticket uuid");
    assert_eq!(seq_of(&read_line(&mut first).await.expect("live1")), 1);
    assert_eq!(seq_of(&read_line(&mut first).await.expect("live2")), 2);
    // 挂起一条 ask 再断流：无人值守 = 立即 reject-once
    send_line(&mut first, &permission_request(77, "execute")).await;
    let forwarded = read_line(&mut first).await.expect("forwarded ask");
    assert!(forwarded.contains("request_permission"), "{forwarded}");
    drop(first);
    wait_audit(
        &r.audit,
        |ev| matches!(ev, AuditEvent::ClientGone { .. }),
        8,
    )
    .await;
    tokio::time::sleep(Duration::from_millis(400)).await; // 窗口内蓄水

    let mut second = open_stream(&r).await;
    let hello = handshake_client_reattach(&mut second, ticket).await;
    assert!(
        matches!(hello, ServerHello::Ready { .. }),
        "reattach must be accepted: {hello:?}",
    );
    send_line(
        &mut second,
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}",
    )
    .await;

    let announce = read_line(&mut second).await.expect("announcement");
    let ann: Value = serde_json::from_str(announce.trim_end()).expect("json");
    assert_eq!(
        ann["method"],
        reattach::REPLAY_ANNOUNCE_METHOD,
        "{announce}"
    );
    let replayed = ann["params"]["replayed"].as_u64().expect("count");
    assert!(replayed >= 1, "window must have cached updates: {announce}");

    let mut last = 2_u64;
    for i in 0..replayed {
        let line = read_line(&mut second).await.expect("replayed update");
        let seq = seq_of(&line);
        assert!(seq > last, "replay order broken at {i}: {seq} after {last}");
        last = seq;
    }
    // 恰好读 replayed 条后即恢复实时透传：宣告条数与补放条数一致的机械证明
    let live = tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let line = read_line(&mut second).await.expect("line after replay");
            if line.contains("\"session/update\"") {
                break line;
            }
        }
    })
    .await
    .expect("live updates resume");
    assert!(seq_of(&live) > last, "live seq must continue: {live}");

    assert!(
        r.audit.contains(|ev| matches!(
            ev,
            AuditEvent::PermissionActed { action, .. } if action == "unanswered-rejected"
        )),
        "detach must reject-once outstanding: {:?}",
        r.audit.snapshot(),
    );
    assert!(
        r.audit.contains(|ev| matches!(
            ev,
            AuditEvent::ReattachAccepted { detail, .. } if *detail == format!("replayed={replayed}")
        )),
        "audited replay count must match announcement: {:?}",
        r.audit.snapshot(),
    );
    shutdown(&r);
}

/// ⑥ 窗口过期→退出阶梯：WindowExpired 审计 + 桩进程退出被观测（SubprocessExit）。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s6_window_expiry_runs_exit_ladder_and_stub_exit_observed() {
    let r = rig("wave-s6", grant(), |cfg| cfg.reattach_window_secs = 1).await;
    let mut first = open_stream(&r).await;
    handshake_client(&mut first).await;
    drop(first);

    wait_audit(
        &r.audit,
        |ev| matches!(ev, AuditEvent::WindowExpired { .. }),
        10,
    )
    .await;
    wait_audit(
        &r.audit,
        |ev| matches!(ev, AuditEvent::SubprocessExit { .. }),
        10,
    )
    .await;
    shutdown(&r);
}

/// ⑦ 超限拒绝（连接门禁两级，设计 §7 资源门禁拍板）：total 可配上限与
/// 每 peer 并发=1；拒绝帧可观察，审计区分 limit=total / per-peer。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s7_caps_denied_total_then_per_peer() {
    let r = rig("wave-s7t", grant(), |cfg| cfg.max_connections = 1).await;
    let mut a = open_stream(&r).await;
    assert!(matches!(
        handshake_client(&mut a).await,
        ServerHello::Ready { .. }
    ));
    let mut b = open_stream(&r).await;
    assert_eq!(
        handshake_client(&mut b).await,
        ServerHello::Denied {
            denied: "conn-cap-reached".to_owned(),
        },
    );
    drop(a);
    drop(b);
    wait_audit(
        &r.audit,
        |ev| matches!(ev, AuditEvent::GateDenied { limit, .. } if *limit == "total"),
        10,
    )
    .await;
    shutdown(&r);

    // per-peer：总数放宽后同 peer 第二条流仍被拒（每 peer 并发=1）
    let r2 = rig("wave-s7p", grant(), |_| {}).await;
    let mut a2 = open_stream(&r2).await;
    assert!(matches!(
        handshake_client(&mut a2).await,
        ServerHello::Ready { .. }
    ));
    let mut b2 = open_stream(&r2).await;
    assert_eq!(
        handshake_client(&mut b2).await,
        ServerHello::Denied {
            denied: "conn-cap-reached".to_owned(),
        },
    );
    drop(a2);
    drop(b2);
    wait_audit(
        &r2.audit,
        |ev| matches!(ev, AuditEvent::GateDenied { limit, .. } if *limit == "per-peer"),
        10,
    )
    .await;
    shutdown(&r2);
}
