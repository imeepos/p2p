//! 保活回归：客户端失联上抛（消融点 2）与服务端静默清理（消融点 3）。

use std::time::Duration;

use crate::support::{ka, manual_reserve, read_frame, spawn_relay, BlackHoleLink};
use p2p_relay::testutil::{mock_link_pair, MockLinkSource};
use p2p_relay::{
    errcode, relay_msg::Kind, write_msg, RelayClient, RelayError, RelayEvent, RelayLimits,
    RelayLink, RelayMsg,
};

/// 保活失联（含消融点 2）：连续超时判失联，WARN+事件上抛，控制面速断。
#[tokio::test]
async fn keepalive_misses_declare_relay_lost_and_fail_fast() {
    let ka = ka(120_000, 60, 60, 2, 45_000);
    let mut client = RelayClient::with_keepalive(Box::new(BlackHoleLink::new("black-hole")), ka);
    // 惰性控制流需先触发：reserve 在黑洞上只建流不等回包。
    let _ = tokio::time::timeout(
        Duration::from_millis(300),
        client.reserve(Duration::from_secs(60), ""),
    )
    .await;
    // 连续 2 次探测无应答 -> 失联事件；消融点 2 删 spawn_keepalive 则 3s 超时变红。
    let ev = tokio::time::timeout(Duration::from_secs(3), client.next_event())
        .await
        .expect("loss must be declared within 3s")
        .expect("event queued");
    match ev {
        RelayEvent::ControlClosed { reason } => {
            assert!(reason.contains("keepalive"), "must be keepalive: {reason}");
        }
        other => panic!("unexpected event: {other:?}"),
    }
    // 失联后控制面速断：不许静默重开控制流装作没事。
    let retry = tokio::time::timeout(
        Duration::from_secs(2),
        client.reserve(Duration::from_secs(60), ""),
    );
    let err = retry
        .await
        .expect("fail-fast must not hang")
        .expect_err("must fail");
    assert!(matches!(err, RelayError::LinkClosed), "got {err:?}");
}

/// 服务端静默清理（含消融点 3）+ 健康保活不误伤反例。
#[tokio::test]
async fn server_silence_timeout_reclaims_and_keepalive_survives() {
    let source = MockLinkSource::new();
    let limits = RelayLimits {
        max_circuits_per_peer: 1,
        ..RelayLimits::default()
    };
    let svc = spawn_relay(&source, limits, ka(120_000, 10_000, 5_000, 3, 300));
    let (cl, sl) = mock_link_pair("silent-a", "silent-a");
    source.push(Box::new(sl));
    let mut ctrl = cl.open_stream().await.expect("open control");
    let cid = manual_reserve(&mut ctrl, 3600, "").await;
    // 消融点 3：裸流不发保活必被清；以保活失败计数收敛替代固定 sleep（无固定时钟）。
    for _ in 0..50_000 {
        if svc.metrics().keepalive_failures_total >= 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(
        svc.metrics().keepalive_failures_total,
        1,
        "静默清理未计入保活失败"
    );
    // ① 被清电路的 connect 得显式 UNKNOWN_CIRCUIT（非挂死、非停车）。
    let mut s = cl.open_stream().await.expect("open circuit stream");
    write_msg(&mut s, &RelayMsg::connect(cid.0))
        .await
        .expect("write connect");
    match read_frame(&mut s).await.kind {
        Some(Kind::Reject(r)) => assert_eq!(r.code, errcode::UNKNOWN_CIRCUIT, "got {r:?}"),
        other => panic!("expected unknown-circuit reject: {other:?}"),
    }
    // ② 健康客户端（50ms 保活）跨 400ms（>1.3× silence，探活持续刷新服务端）
    // 不得被清；失败计数相对基线零增量（此时无在途裸流，计数可隔离归因）。
    let (hl, hs) = mock_link_pair("healthy-b", "healthy-b");
    source.push(Box::new(hs));
    let mut h = RelayClient::with_keepalive(Box::new(hl), ka(120_000, 50, 5_000, 3, 300));
    let failures_before = svc.metrics().keepalive_failures_total;
    h.reserve(Duration::from_secs(3600), "")
        .await
        .expect("healthy register");
    tokio::time::sleep(Duration::from_millis(400)).await;
    // 若被清服务端 EOF 必达客户端成 ControlClosed 事件；无事件即保活生效
    let ev = tokio::time::timeout(Duration::from_millis(500), h.next_event()).await;
    assert!(ev.is_err(), "healthy control must survive silence: {ev:?}");
    assert_eq!(
        svc.metrics().keepalive_failures_total,
        failures_before,
        "健康客户端不得新增保活失败"
    );
    // ③ 配额回吐：同 Peer 换新控制流可再次注册（否则 PEER_LIMIT）。
    let mut ctrl2 = cl.open_stream().await.expect("reopen control");
    let _ = manual_reserve(&mut ctrl2, 3600, "").await;
}
