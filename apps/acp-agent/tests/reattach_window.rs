//! ACP4 续连窗口边界回环集成测试（设计 §5/§7）：窗口过期走退出阶梯、
//! 无票据新连接顶替遗留槽位（孤儿进程不过夜）、失窃票据永不放行第二设备。

mod common;

use std::time::Duration;

use acp_agent::AuditEvent;
use acp_common::{AskRoute, Scope, ServerHello};
use common::{
    handshake_client, handshake_client_reattach, open_stream, shutdown, test_config,
    test_grant_full, Rig,
};
use uuid::Uuid;

async fn rig_window(tag: &str, window: u64) -> Rig {
    let client = common::build_client(tag).await;
    let mut cfg = test_config(tag);
    cfg.reattach_window_secs = window;
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn window_expiry_runs_exit_ladder() {
    let rig = rig_window("re-expire", 1).await;
    let mut first = open_stream(&rig).await;
    handshake_client(&mut first).await;
    drop(first);

    // 窗口过期 => 退出阶梯 => 子进程退出审计
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    while !rig
        .audit
        .contains(|ev| matches!(ev, AuditEvent::WindowExpired { .. }))
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "window expiry not observed: {:?}",
            rig.audit.snapshot(),
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let exited = tokio::time::Instant::now() + Duration::from_secs(8);
    while !rig
        .audit
        .contains(|ev| matches!(ev, AuditEvent::SubprocessExit { .. }))
    {
        assert!(
            tokio::time::Instant::now() < exited,
            "subprocess exit not observed: {:?}",
            rig.audit.snapshot(),
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    shutdown(&rig);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fresh_connection_supersedes_stale_window_slot() {
    let rig = rig_window("re-super", 30).await;
    let mut first = open_stream(&rig).await;
    handshake_client(&mut first).await;
    drop(first);
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 无票据新连接 = 放弃续连：顶替遗留槽位并正常建立
    let mut second = open_stream(&rig).await;
    let hello = handshake_client(&mut second).await;
    assert!(matches!(hello, ServerHello::Ready { .. }));
    assert!(
        rig.audit
            .contains(|ev| matches!(ev, AuditEvent::SlotSuperseded { .. })),
        "supersede must be audited: {:?}",
        rig.audit.snapshot(),
    );
    let exited = tokio::time::Instant::now() + Duration::from_secs(8);
    while rig
        .audit
        .snapshot()
        .iter()
        .filter(|ev| matches!(ev, AuditEvent::SubprocessExit { .. }))
        .count()
        < 1
    {
        assert!(
            tokio::time::Instant::now() < exited,
            "superseded subprocess must exit: {:?}",
            rig.audit.snapshot(),
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    drop(second);
    shutdown(&rig);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stolen_ticket_never_grants_second_device() {
    let rig = rig_window("re-cross", 30).await;
    let mut first = open_stream(&rig).await;
    let ServerHello::Ready { ready } = handshake_client(&mut first).await else {
        panic!("ready expected")
    };
    let stolen = Uuid::parse_str(&ready.ticket.expect("ticket")).expect("ticket uuid");
    drop(first);

    // 原设备断链（运输层断开事件由底座收敛，时序不定）；另一设备持失窃票据重连。
    // 安全不变量：无论归属 fail-closed（peer-not-allowed）还是票据绑定校验
    // （reattach-ticket-invalid），失窃票据都拿不到任何会话。
    rig.client.shutdown();
    let attacker = common::build_client("re-cross-attacker").await;
    common::seed_quic(&rig.server, rig.server_peer, &attacker);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut attempts = 0;
    while tokio::time::Instant::now() < deadline && attempts < 8 {
        if attacker.connect(rig.server_peer).await.is_err() {
            tokio::time::sleep(Duration::from_millis(200)).await;
            continue;
        }
        let mut stream = attacker
            .new_stream(
                rig.server_peer,
                p2p::ProtocolId::new(common::PROTO).expect("proto"),
            )
            .await
            .expect("open stream");
        let hello = handshake_client_reattach(&mut stream, stolen).await;
        assert!(
            matches!(hello, ServerHello::Denied { .. }),
            "stolen ticket must never yield ready, got {hello:?}",
        );
        drop(stream);
        attempts += 1;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(attempts > 0, "attacker must have attempted at least once");
    assert!(
        !rig.audit
            .contains(|ev| matches!(ev, AuditEvent::ReattachAccepted { .. })),
        "stolen ticket must not be accepted: {:?}",
        rig.audit.snapshot(),
    );
    attacker.shutdown();
    shutdown(&rig);
}
