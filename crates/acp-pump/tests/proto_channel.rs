//! ?proto= 协议感知拨号与 a2a 卡片事件通道（gui-contract §17）：
//! proto=a2a 的 WS 连接拨 /a2a/1 流（字节透传即事件通道本体），
//! proto 缺省 acp、未知值 401 显式拒绝；对端桩协议 ID 单一化断言。

mod common;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::tungstenite::Bytes;

use acp_common::{frames, LineReassembler};
use acp_pump::dial::{dial_and_handshake, DialProto};
use acp_pump::state::{ConnPhase, StatusHub};
use p2p_protocol::{read_frame, write_frame};
use tokio::io::AsyncWriteExt;

use common::*;

/// 等待状态机进入指定相位（与 loopback 同口径：显式超时，禁无界等待）。
async fn wait_phase(hub: &StatusHub, target: ConnPhase) {
    let mut rx = hub.subscribe();
    let deadline = tokio::time::Instant::now() + STEP;
    loop {
        if rx.borrow().phase == target {
            return;
        }
        let now = tokio::time::Instant::now();
        assert!(now < deadline, "timeout waiting for phase {target:?}");
        match tokio::time::timeout(deadline - now, rx.changed()).await {
            Ok(Ok(())) => {}
            other => panic!("phase wait failed: {other:?}"),
        }
    }
}

/// proto=a2a 拨号：对端桩注册 /a2a/1，拨通且 card 相帧 echo 往返（透传语义）。
#[tokio::test]
async fn a2a_proto_dial_reaches_a2a_endpoint() {
    let rig = rig("a2a-dial", AgentMock::echo_a2a()).await;
    let (_, outcome, mut stream) =
        dial_and_handshake(&rig.console, rig.agent_peer, DialProto::A2a, None, None)
            .await
            .expect("a2a dial should reach /a2a/1 endpoint");
    assert!(outcome.ticket.is_none());
    let line = b"{\"op\":\"list\",\"v\":1,\"id\":1}";
    for frame in frames(line) {
        write_frame(&mut stream, frame).await.unwrap();
    }
    stream.flush().await.unwrap();
    let mut reassembler = LineReassembler::new();
    let echo = loop {
        if let Some(line) = reassembler.take_line() {
            break line;
        }
        let frame = tokio::time::timeout(STEP, read_frame(&mut stream))
            .await
            .unwrap()
            .unwrap();
        reassembler.push_frame(&frame).unwrap();
    };
    // echo 泵整行（含行尾换行）原样回写，与 loopback 口径一致。
    assert_eq!(echo, b"{\"op\":\"list\",\"v\":1,\"id\":1}\n");
    teardown(rig);
}

/// 协议错配：/a2a/1 对端桩上按 acp 拨号必须显式失败（无 handler 可拨）。
#[tokio::test]
async fn acp_proto_dial_fails_on_a2a_only_endpoint() {
    let rig = rig("a2a-mismatch", AgentMock::echo_a2a()).await;
    let result = dial_and_handshake(&rig.console, rig.agent_peer, DialProto::Acp, None, None).await;
    assert!(result.is_err(), "acp dial on a2a-only endpoint must fail");
    teardown(rig);
}

/// WS ?proto=a2a 连接成功并进入 Online；随后字节透传（事件通道 = 哑泵语义）。
#[tokio::test]
async fn ws_proto_a2a_connects_and_pumps() {
    let rig = rig("a2a-ws", AgentMock::echo_a2a()).await;
    let mut ws = ws_try_connect_with(
        &rig,
        &format!("token={}&peer={}&proto=a2a", rig.token, rig.agent_peer),
    )
    .await
    .expect("proto=a2a ws connect");
    wait_phase(&rig.hub, ConnPhase::Online).await;
    // ndjson 行必须有行尾换行：对端桩按行重组（无换行不成行，静默积压）。
    ws.send(Message::Binary(Bytes::from_static(
        b"{\"op\":\"subscribe\",\"v\":1,\"id\":3}\n",
    )))
    .await
    .unwrap();
    let msg = tokio::time::timeout(STEP, ws.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        msg.into_data().as_ref(),
        b"{\"op\":\"subscribe\",\"v\":1,\"id\":3}\n"
    );
    teardown(rig);
}

/// WS ?proto=未知值：401 显式拒绝（禁静默回落 acp）。
#[tokio::test]
async fn ws_rejects_unknown_proto() {
    let rig = rig("a2a-badproto", AgentMock::echo_a2a()).await;
    match ws_try_connect_with(
        &rig,
        &format!("token={}&peer={}&proto=grpc", rig.token, rig.agent_peer),
    )
    .await
    {
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
            assert_eq!(resp.status(), 401);
        }
        other => panic!("expected 401 for unknown proto, got {other:?}"),
    }
    teardown(rig);
}
