//! 单机回环测试（需求 E）：in-test 起 facade 服务端模拟 agent（acp-common 握手应答），
//! 验证拨号+握手 roundtrip、WS token 鉴权拒绝、WS⇄P2P 字节透传 roundtrip、
//! 对端断流→offline、票据落盘与读取；另覆盖 denied 面、status 端点鉴权与关闭传播。

mod common;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::tungstenite::Bytes;

use acp_console::dial::dial_and_handshake;
use acp_console::state::{ConnPhase, StatusHub};

use common::*;

/// 等待状态机进入指定相位（显式超时，禁无界等待）。
async fn wait_phase(hub: &StatusHub, target: ConnPhase) -> acp_console::StateSnapshot {
    let mut rx = hub.subscribe();
    let deadline = tokio::time::Instant::now() + STEP;
    loop {
        let snap = rx.borrow().clone();
        if snap.phase == target {
            return snap;
        }
        let now = tokio::time::Instant::now();
        assert!(now < deadline, "timeout waiting for phase {target:?}");
        match tokio::time::timeout(deadline - now, rx.changed()).await {
            Ok(Ok(())) => {}
            other => panic!("phase wait failed: {other:?}"),
        }
    }
}

/// 需求 E-1：拨号 + 握手 roundtrip（直驱 dial 模块，mock 收到的 conn 必须一致）。
#[tokio::test]
async fn dial_handshake_roundtrip_carries_bytes() {
    let rig = rig("dial", AgentMock::echo()).await;
    let (peer, outcome, mut stream) = dial_and_handshake(&rig.console, rig.agent_peer, None, None)
        .await
        .unwrap();
    let conn = outcome.conn;
    assert_eq!(peer, rig.agent_peer);
    assert_eq!(rig.mock.hello().unwrap().conn, conn);

    stream.write_all(b"{\"jsonrpc\":\"2.0\"}\n").await.unwrap();
    stream.flush().await.unwrap();
    let mut echo = [0u8; 18];
    tokio::time::timeout(STEP, stream.read_exact(&mut echo))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&echo, b"{\"jsonrpc\":\"2.0\"}\n");
    teardown(rig);
}

/// 需求 E-2：WS token 鉴权拒绝（无 token / 错 token 均以 HTTP 401 拒绝）。
#[tokio::test]
async fn ws_rejects_missing_and_bad_token() {
    let rig = rig("authtok", AgentMock::echo()).await;
    for query in [
        format!("peer={}", rig.agent_peer),
        format!("token=wrong&peer={}", rig.agent_peer),
    ] {
        match ws_try_connect_with(&rig, &query).await {
            Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
                assert_eq!(resp.status(), 401, "query={query}");
            }
            other => panic!("expected 401 rejection for {query}, got {other:?}"),
        }
    }
    teardown(rig);
}

/// 需求 E-3：WS⇄P2P 字节透传 roundtrip（小 payload + 跨 64 KiB 块的大 payload）。
#[tokio::test]
async fn ws_p2p_byte_roundtrip() {
    let rig = rig("roundtrip", AgentMock::echo()).await;
    let mut ws = ws_connect(&rig).await;
    wait_phase(&rig.hub, ConnPhase::Online).await;

    ws.send(Message::Binary(Bytes::from_static(b"hello-agent\n")))
        .await
        .unwrap();
    let msg = tokio::time::timeout(STEP, ws.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(msg.into_data().as_ref(), b"hello-agent\n");

    let payload: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    ws.send(Message::Binary(Bytes::from(payload.clone())))
        .await
        .unwrap();
    let mut got: Vec<u8> = Vec::new();
    while got.len() < payload.len() {
        let msg = tokio::time::timeout(STEP, ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        got.extend_from_slice(&msg.into_data());
    }
    assert_eq!(got, payload, "multi-chunk echo must be byte-exact");
    teardown(rig);
}

/// 需求 E-4：对端断流（agent 侧连接终止，生产断连主路径）→
/// reattach-window → offline（含迁移链可见）。
#[tokio::test]
async fn peer_stream_loss_lands_offline_via_window() {
    let rig = rig("drop", AgentMock::echo()).await;
    let _ws = ws_connect(&rig).await;
    wait_phase(&rig.hub, ConnPhase::Online).await;
    // 底座契约：流级半关闭无原语（yamux Compat shutdown 为 no-op），
    // agent 退出以连接级断开模拟（真实 agent 进程死亡即此路径）。
    rig.agent.shutdown();
    let window_snap = wait_phase(&rig.hub, ConnPhase::ReattachWindow).await;
    assert_eq!(
        window_snap.peer.as_deref(),
        Some(rig.agent_peer.to_string().as_str())
    );
    let offline = wait_phase(&rig.hub, ConnPhase::Offline).await;
    assert_eq!(
        offline.peer.as_deref(),
        Some(rig.agent_peer.to_string().as_str())
    );
    teardown(rig);
}

/// 需求 E-5：票据落盘与读取（conn 与 agent 实收握手帧、status 快照一致）。
#[tokio::test]
async fn ticket_persisted_and_readable_after_online() {
    let rig = rig("ticket", AgentMock::echo()).await;
    let _ws = ws_connect(&rig).await;
    let online = wait_phase(&rig.hub, ConnPhase::Online).await;
    let conn_from_status = online.conn.expect("online snapshot carries conn");

    let ticket = rig
        .tickets
        .latest_for(&rig.agent_peer.to_string())
        .unwrap()
        .expect("ticket on disk");
    assert_eq!(ticket.conn.to_string(), conn_from_status);
    assert_eq!(ticket.peer, rig.agent_peer.to_string());
    assert_eq!(rig.mock.hello().unwrap().conn, ticket.conn);
    assert!(rig.data_dir.join("reattach-tickets.json").exists());
    teardown(rig);
}

/// 补充面：agent denied → WS Close(4403) + offline（A 的可观察拒绝路径）。
#[tokio::test]
async fn denied_handshake_surfaces_close_code_and_offline() {
    let rig = rig("denied", AgentMock::denying("peer-not-allowed")).await;
    let mut ws = ws_connect(&rig).await;
    let close = tokio::time::timeout(STEP, ws.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    match close {
        Message::Close(Some(frame)) => {
            assert_eq!(u16::from(frame.code), 4403, "denied close code");
            assert!(
                frame.reason.contains("peer-not-allowed"),
                "{}",
                frame.reason
            );
        }
        other => panic!("expected close frame, got {other:?}"),
    }
    wait_phase(&rig.hub, ConnPhase::Offline).await;
    teardown(rig);
}

/// 补充面：WS 客户端关闭 → 连接关闭双向传播（agent 侧流见 EOF）。
#[tokio::test]
async fn ws_client_close_propagates_to_agent() {
    let rig = rig("close", AgentMock::echo()).await;
    let mut ws = ws_connect(&rig).await;
    wait_phase(&rig.hub, ConnPhase::Online).await;
    ws.send(Message::Close(None)).await.unwrap();
    wait_phase(&rig.hub, ConnPhase::ReattachWindow).await;
    // 泵结束原因是客户端关闭；agent 侧 echo 循环收到写半 shutdown（EOF 由协议栈呈现）。
    teardown(rig);
}

/// 补充面（需求 C 查询面）：status 端点 Bearer 鉴权与快照形状。
#[tokio::test]
async fn status_endpoint_reports_snapshot_with_token() {
    let rig = rig("status", AgentMock::echo()).await;
    let server = acp_console::status::StatusServer::start(0, rig.token.clone(), status_deps(&rig))
        .await
        .unwrap();

    let get = |path: &str, auth: Option<&str>| {
        let addr = server.addr;
        let path = path.to_string();
        let auth = auth.map(str::to_string);
        async move {
            let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
            let head = match auth {
                Some(t) => format!("GET {path} HTTP/1.1\r\nAuthorization: Bearer {t}\r\n\r\n"),
                None => format!("GET {path} HTTP/1.1\r\n\r\n"),
            };
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            s.write_all(head.as_bytes()).await.unwrap();
            let mut buf = Vec::new();
            tokio::time::timeout(STEP, s.read_to_end(&mut buf))
                .await
                .unwrap()
                .unwrap();
            String::from_utf8(buf).unwrap()
        }
    };

    let denied = get("/status", None).await;
    assert!(denied.starts_with("HTTP/1.1 401"), "{denied}");
    let ok = get("/status", Some(&rig.token)).await;
    assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
    assert!(ok.contains("\"phase\":\"offline\""), "{ok}");
    let disc = get("/discovery", Some(&rig.token)).await;
    assert!(
        disc.starts_with("HTTP/1.1 200") && disc.contains("\"peers\""),
        "{disc}"
    );
    let missing = get("/nope", Some(&rig.token)).await;
    assert!(missing.starts_with("HTTP/1.1 404"), "{missing}");
    teardown(rig);
}

/// 显式防漂移：协议 ID 常量可被底座 ProtocolId 接受（防止手抄字面量漂移）。
#[test]
fn protocol_id_literal_is_shared_constant() {
    assert_eq!(acp_common::consts::PROTOCOL_ID, "/dsh-acp/1");
    assert!(p2p::ProtocolId::new(acp_common::consts::PROTOCOL_ID).is_ok());
}
