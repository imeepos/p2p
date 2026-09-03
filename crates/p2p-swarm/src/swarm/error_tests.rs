//! E9-T3 错误链保真回归：逐点位断言 source() 链可达内层错误。
//! 消融证明：任一点位改回 `other(e.to_string())` 拍平写法，对应测试即红
//! （source 变 None，downcast 失败）。

use std::error::Error;
use std::io;
use std::time::Duration;

use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_relay::{PunchReq, PunchSession, RelayClient, RelayError, RelayLink};
use p2p_security::SecurityError;
use p2p_transport::TransportAddr;

use super::degrade::{degrade, join_circuit, secure_outbound, stage_on_ack, stage_request_sent};
use super::dial::dial_one;
use super::relay_session::dial_relay_link;
use super::responder::secure_inbound;
use super::tests::test_config;
use super::Swarm;

/// open_stream 恒失败的 mock 链路：驱动 reserve/connect 走失败路径。
struct FailingLink;

#[async_trait::async_trait]
impl RelayLink for FailingLink {
    fn peer_id(&self) -> &str {
        "mock-relay"
    }

    async fn open_stream(&self) -> io::Result<BoxedStream> {
        Err(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            "mock link refused",
        ))
    }

    async fn accept_stream(&self) -> Option<BoxedStream> {
        None
    }
}

/// 本机未监听的 TCP 端口：loopback 拒绝即时返回（macOS 实测，见 tests.rs）。
fn dead_tcp_addr() -> TransportAddr {
    TransportAddr::Tcp {
        ip: "127.0.0.1".parse().expect("valid ip"),
        port: 1,
    }
}

async fn test_swarm() -> std::sync::Arc<Swarm> {
    Swarm::start(test_config()).await.expect("bind swarm")
}

/// Ok 类型（Mux 等）不带 Debug，不能 expect_err，统一用 match 取 Err。
fn take_err<T>(res: io::Result<T>, why: &str) -> io::Error {
    match res {
        Ok(_) => panic!("{why}"),
        Err(e) => e,
    }
}

/// 断言拨号链保真：swarm 包装 → TransportError → 内层 io::Error 全链可遍历。
fn assert_dial_chain(err: &io::Error) {
    let te = err
        .source()
        .expect("source must be kept, not flattened")
        .downcast_ref::<p2p_transport::TransportError>()
        .expect("TransportError reachable");
    let inner = te
        .source()
        .expect("transport keeps dial source")
        .downcast_ref::<io::Error>()
        .expect("inner io::Error");
    assert_eq!(
        inner.kind(),
        io::ErrorKind::ConnectionRefused,
        "refused must survive two wrap layers"
    );
}

/// 断言错误保真：source 存在且能还原为指定内层类型。
fn assert_source<T: Error + 'static>(err: &io::Error) {
    let src = err.source().expect("source must be kept, not flattened");
    assert!(
        src.downcast_ref::<T>().is_some(),
        "inner {} must be reachable via source(), got {err}",
        std::any::type_name::<T>()
    );
}

/// 点位 dial.rs dial_one：传输层拨号错误以原 io::Error 挂 source。
#[tokio::test]
async fn dial_one_keeps_transport_error_source() {
    let swarm = test_swarm().await;
    let peer = PeerId::from_bytes([9; 32]);
    let err = take_err(
        dial_one(&swarm, peer, &dead_tcp_addr()).await,
        "port 1 must refuse",
    );
    assert_dial_chain(&err);
}

/// 点位 relay_session dial_relay_link：relay 链路拨号不拍平。
#[tokio::test]
async fn dial_relay_link_keeps_transport_error_source() {
    let swarm = test_swarm().await;
    let err = take_err(
        dial_relay_link(&swarm, &dead_tcp_addr()).await,
        "port 1 must refuse",
    );
    assert_dial_chain(&err);
}

/// 点位 degrade reserve_circuit：RelayError 挂 source。
#[tokio::test]
async fn degrade_reserve_failure_keeps_relay_error_source() {
    let swarm = test_swarm().await;
    let peer = PeerId::from_bytes([9; 32]);
    let mut client = RelayClient::new(Box::new(FailingLink));
    let err = take_err(
        degrade(&swarm, &mut client, peer).await,
        "mock link must fail reserve",
    );
    assert_source::<RelayError>(&err);
}

/// 点位 stage_request_sent：非法状态推进的 RelayError 保 source
/// （应答侧会话处于 AckSent，mark_request_sent 必拒绝）。
#[test]
fn stage_request_sent_keeps_relay_error_source() {
    let req = PunchReq {
        peer_id: "peer-a".to_string(),
        addrs: Vec::new(),
    };
    let mut session = PunchSession::responder(&req);
    let err = stage_request_sent(&mut session).expect_err("responder cannot send request");
    assert_source::<RelayError>(&err);
}

/// 点位 stage_on_ack：AckSent 相位收 Ack 的 RelayError 保 source。
#[test]
fn stage_on_ack_keeps_relay_error_source() {
    let req = PunchReq {
        peer_id: "peer-a".to_string(),
        addrs: Vec::new(),
    };
    let mut session = PunchSession::responder(&req);
    let err = stage_on_ack(&mut session).expect_err("ack before request must fail");
    assert_source::<RelayError>(&err);
}

/// 点位 join_circuit：电路接入失败的 RelayError 保 source。
#[tokio::test]
async fn join_circuit_keeps_relay_error_source() {
    let mut client = RelayClient::new(Box::new(FailingLink));
    let err = take_err(
        join_circuit(&mut client, 7).await,
        "mock link must refuse circuit join",
    );
    assert_source::<RelayError>(&err);
}

/// 点位 degrade secure_outbound：发起侧握手对死流失败保 source。
#[tokio::test]
async fn secure_outbound_keeps_handshake_error_source() {
    let swarm = test_swarm().await;
    let peer = PeerId::from_bytes([9; 32]);
    let (stream, dead) = tokio::io::duplex(64);
    drop(dead); // 对端即刻消失：握手读写必然失败
    let res = tokio::time::timeout(
        Duration::from_secs(5),
        secure_outbound(&swarm, Box::new(stream), peer),
    )
    .await;
    let err = match res {
        Err(_) => panic!("handshake must fail fast on dead stream"),
        Ok(Ok(_)) => panic!("dead peer must fail handshake"),
        Ok(Err(e)) => e,
    };
    assert_source::<SecurityError>(&err);
}

/// 点位 responder secure_inbound：被动握手对死流失败保 source。
#[tokio::test]
async fn secure_inbound_keeps_handshake_error_source() {
    let swarm = test_swarm().await;
    let (stream, dead) = tokio::io::duplex(64);
    drop(dead);
    let res = tokio::time::timeout(
        Duration::from_secs(5),
        secure_inbound(&swarm, Box::new(stream)),
    )
    .await;
    let err = match res {
        Err(_) => panic!("handshake must fail fast on dead stream"),
        Ok(Ok(_)) => panic!("dead stream must fail inbound handshake"),
        Ok(Err(e)) => e,
    };
    assert_source::<SecurityError>(&err);
}
