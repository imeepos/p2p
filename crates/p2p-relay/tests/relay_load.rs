//! 负载水位广播：Reserved/KeepAliveAck 的 load_permille 必须随服务端占用上升。

use std::time::Duration;

use p2p_mux::BoxedStream;
use p2p_relay::relay_msg::Kind;
use p2p_relay::testutil::{mock_link_pair, MockLinkSource};
use p2p_relay::{read_msg, write_msg, RelayLimits, RelayLink, RelayMsg, RelayServiceImpl};

/// 限流小电路池：让电路占用成为主导资源，断言确定水位。
fn limits_two_circuits() -> RelayLimits {
    RelayLimits {
        max_total_circuits: 2,
        ..RelayLimits::default()
    }
}

async fn ask(ctrl: &mut BoxedStream, msg: RelayMsg, what: &'static str) -> RelayMsg {
    write_msg(ctrl, &msg)
        .await
        .unwrap_or_else(|e| panic!("{what}: write failed: {e}"));
    match tokio::time::timeout(Duration::from_secs(2), read_msg(ctrl)).await {
        Ok(Ok(Some(reply))) => reply,
        Ok(Ok(None)) => panic!("{what}: control stream closed"),
        Ok(Err(e)) => panic!("{what}: read failed: {e}"),
        Err(_) => panic!("{what}: reply timeout"),
    }
}

fn reserved_load(reply: &RelayMsg, what: &'static str) -> u32 {
    match reply.kind.as_ref() {
        Some(Kind::Reserved(r)) => r.load_permille,
        Some(Kind::Reject(r)) => panic!("{what}: rejected code={} {}", r.code, r.message),
        other => panic!("{what}: unexpected frame {other:?}"),
    }
}

fn ack_load(reply: &RelayMsg, what: &'static str) -> u32 {
    match reply.kind.as_ref() {
        Some(Kind::KeepAliveAck(a)) => a.load_permille,
        other => panic!("{what}: unexpected frame {other:?}"),
    }
}

#[tokio::test]
async fn load_permille_rises_with_circuit_occupancy() {
    let source = MockLinkSource::new();
    let _svc = RelayServiceImpl::spawn(Box::new(source.clone()), limits_two_circuits());
    let (client_side, server_side) = mock_link_pair("peer-a", "relay");
    source.push(Box::new(server_side));
    let mut ctrl = client_side
        .open_stream()
        .await
        .expect("open control stream");

    // 1/2 电路在册：电路 500‰ 为最弱资源
    let r1 = ask(&mut ctrl, RelayMsg::reserve(3600, ""), "reserve-1").await;
    assert_eq!(reserved_load(&r1, "reserve-1"), 500);

    let a1 = ask(&mut ctrl, RelayMsg::keep_alive(), "keepalive-1").await;
    assert_eq!(ack_load(&a1, "keepalive-1"), 500);

    // 2/2 打满：水位 1000‰
    let r2 = ask(&mut ctrl, RelayMsg::reserve(60, "peer-b"), "reserve-2").await;
    assert_eq!(reserved_load(&r2, "reserve-2"), 1000);

    let a2 = ask(&mut ctrl, RelayMsg::keep_alive(), "keepalive-2").await;
    assert_eq!(ack_load(&a2, "keepalive-2"), 1000);
}
