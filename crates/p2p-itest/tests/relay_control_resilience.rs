//! E4 回归（relay 控制面韧性）：控制注册不再泄漏配额。
//!
//! 缺陷背景：每次重连的控制注册内含 reserve（TTL 最长 3600s），控制流关闭后
//! 槽位不回收，churn 下按 per-peer 配额（32）滚动自锁（实测 25s 内 32 满）。
//! 修复语义：控制流存亡即注册载体存亡；等配对方拿显式拒绝而非悬挂超时。

use std::time::Duration;

use p2p_itest::expect_within;
use p2p_relay::testutil::{mock_link_pair, MockLinkSource};
use p2p_relay::{errcode, RelayClient, RelayError, RelayLimits, RelayServiceImpl};

const LIMIT: Duration = Duration::from_secs(10);
/// 超过 per-peer 配额（32）的轮数：修复前第 33 轮必锁。
const CHURN_ROUNDS: usize = 40;
const CHURN_GAP: Duration = Duration::from_millis(20);

fn spawn_relay(limits: RelayLimits) -> MockLinkSource {
    let source = MockLinkSource::new();
    RelayServiceImpl::spawn(Box::new(source.clone()), limits);
    source
}

/// churn 回归：每轮新控制流注册（TTL 3600s），关闭即回收；40 轮全部成功。
#[tokio::test]
async fn reconnect_churn_never_exhausts_circuit_quota() {
    let source = spawn_relay(RelayLimits::default());
    for round in 0..CHURN_ROUNDS {
        let (client_link, server_link) = mock_link_pair("churn-a", "churn-a");
        source.push(Box::new(server_link));
        let mut client = RelayClient::new(Box::new(client_link));
        let cid = expect_within(
            "churn reserve",
            client.reserve(Duration::from_secs(3600), ""),
            LIMIT,
        )
        .await
        .unwrap_or_else(|e| panic!("round {round} reserve failed: {e}"));
        assert_ne!(cid.0, 0);
        drop(client);
        tokio::time::sleep(CHURN_GAP).await;
    }
}

/// owner 控制流断开后，等配对的接入方拿到显式 CIRCUIT_EXPIRED 拒绝。
#[tokio::test]
async fn parked_joiner_gets_explicit_reject_when_control_dies() {
    let source = spawn_relay(RelayLimits::default());

    let (owner_link, owner_server) = mock_link_pair("owner-a", "owner-a");
    source.push(Box::new(owner_server));
    let mut owner = RelayClient::new(Box::new(owner_link));

    let (joiner_link, joiner_server) = mock_link_pair("joiner-b", "joiner-b");
    source.push(Box::new(joiner_server));
    let mut joiner = RelayClient::new(Box::new(joiner_link));

    let cid = expect_within(
        "owner reserve",
        owner.reserve(Duration::from_secs(60), "joiner-b"),
        LIMIT,
    )
    .await
    .expect("owner reserve");

    // connect 是惰性 future：先主动推进数步让 Connect 落地并 park，
    // 再断 owner 控制流——断言对象才是 parked 拒绝路径而非 UNKNOWN_CIRCUIT
    let joiner_connect = joiner.connect(cid);
    tokio::pin!(joiner_connect);
    for _ in 0..10 {
        let _ = futures::poll!(joiner_connect.as_mut());
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    drop(owner);
    let outcome = expect_within("parked joiner outcome", joiner_connect, LIMIT).await;
    match outcome {
        Err(RelayError::Server { code, .. }) => {
            assert_eq!(
                code,
                errcode::CIRCUIT_EXPIRED,
                "must be explicit expiry reject"
            );
        }
        Ok(_) => panic!("parked circuit must not pair after control close"),
        Err(other) => panic!("expected Server reject, got {other:?}"),
    }
}
