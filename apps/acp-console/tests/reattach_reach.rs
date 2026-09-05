//! 目标一验收（console 侧可达性闭环）：ready 票据捕获落盘 → 断流窗口内
//! /reattach 可查（reason=ok）→ 带票据重连被 agent 实收；窗口过期后如实报
//! expired 且不返回票据。票据用 uuid 形态，与桥签发约定（apps/acp-agent/README.md）同形。

mod common;

use std::sync::Arc;

use futures_util::SinkExt;
use tokio_tungstenite::tungstenite::protocol::Message;
use uuid::Uuid;

use acp_console::dial::dial_and_handshake;
use acp_console::state::ConnPhase;

use common::*;

/// 等待状态机进入指定相位：250ms 轮询快照（客户端断流后的窗口迁移伴随
/// tungstenite 关闭握手，changed() 等待在该路径上不稳定，轮询为探针验证形态）。
async fn wait_phase(hub: &acp_console::state::StatusHub, target: ConnPhase) {
    let deadline = tokio::time::Instant::now() + STEP;
    loop {
        let snap = hub.snapshot();
        if snap.phase == target {
            return;
        }
        eprintln!(
            "[poll] cur={:?} detail={:?} want={target:?}",
            snap.phase, snap.detail
        );
        assert!(
            tokio::time::Instant::now() < deadline,
            "timeout waiting for phase {target:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}

async fn start_status(rig: &Rig) -> acp_console::status::StatusServer {
    acp_console::status::StatusServer::start(0, rig.token.clone(), status_deps(rig))
        .await
        .unwrap()
}

/// 闭环：断流 → 窗口内 /reattach 返回桥票据 → 带票据重连被 agent 实收。
/// （GUI 波等价面见 apps/gui vitest stub 回环；真机 e2e 不具备，本测覆盖 console 半环。）
#[tokio::test]
async fn reattach_ticket_reachable_within_window_and_accepted_on_reconnect() {
    let bridge_ticket = Uuid::new_v4().to_string();
    let rig = rig("reach", AgentMock::echo_with_ticket(&bridge_ticket)).await;
    let mut ws = ws_connect(&rig).await;
    wait_phase(&rig.hub, ConnPhase::Online).await;

    let saved = rig
        .tickets
        .latest_for(&rig.agent_peer.to_string())
        .unwrap()
        .expect("ticket on disk");
    assert_eq!(saved.ticket.as_deref(), Some(bridge_ticket.as_str()));

    // 客户端断流（WS Close）→ reattach-window：票据登记断流锚点，窗口内可查
    ws.send(Message::Close(None)).await.unwrap();
    wait_phase(&rig.hub, ConnPhase::ReattachWindow).await;
    let server = start_status(&rig).await;
    let peer = rig.agent_peer.to_string();
    let ok = http_get(
        server.addr,
        &format!("/reattach?peer={peer}"),
        Some(&rig.token),
    )
    .await;
    assert!(ok.contains("\"reason\":\"ok\""), "{ok}");
    assert!(!ok.contains("\"ticket\":null"), "{ok}");

    // 带票据重连：agent 实收握手行的 reattach 字段 == 桥签发票据
    let reattach = Uuid::parse_str(&bridge_ticket).unwrap();
    let (_, _, _stream) = dial_and_handshake(&rig.console, rig.agent_peer, None, Some(reattach))
        .await
        .unwrap();
    assert_eq!(rig.mock.hello().unwrap().reattach, Some(reattach));
    teardown(rig);
}

/// 过期如实反映：窗口经过后 /reattach 报 expired，且不返回过期票据。
#[tokio::test]
async fn reattach_query_reports_expired_after_window() {
    let bridge_ticket = Uuid::new_v4().to_string();
    let rig = rig("expire", AgentMock::echo_with_ticket(&bridge_ticket)).await;
    eprintln!("[hubdbg] test rig.hub ptr={:p}", Arc::as_ptr(&rig.hub));
    let mut ws = ws_connect(&rig).await;
    wait_phase(&rig.hub, ConnPhase::Online).await;
    ws.send(Message::Close(None)).await.unwrap();
    wait_phase(&rig.hub, ConnPhase::ReattachWindow).await;
    tokio::time::sleep(TEST_WINDOW + std::time::Duration::from_millis(250)).await;
    wait_phase(&rig.hub, ConnPhase::Offline).await;

    let server = start_status(&rig).await;
    let peer = rig.agent_peer.to_string();
    let body = http_get(
        server.addr,
        &format!("/reattach?peer={peer}"),
        Some(&rig.token),
    )
    .await;
    assert!(body.contains("\"reason\":\"expired\""), "{body}");
    assert!(body.contains("\"ticket\":null"), "{body}");
    assert!(
        !body.contains(&bridge_ticket),
        "expired ticket leaked: {body}"
    );
    teardown(rig);
}

/// 契约护栏：缺参 400、无 token 401、未知 peer missing。
#[tokio::test]
async fn reattach_endpoint_contract_guards() {
    let rig = rig("guard", AgentMock::echo()).await;
    let server = start_status(&rig).await;
    let no_token = http_get(server.addr, "/reattach?peer=x", None).await;
    assert!(no_token.starts_with("HTTP/1.1 401"), "{no_token}");
    let missing_peer = http_get(server.addr, "/reattach", Some(&rig.token)).await;
    assert!(missing_peer.starts_with("HTTP/1.1 400"), "{missing_peer}");
    let unknown = http_get(
        server.addr,
        &format!("/reattach?peer={}", Uuid::new_v4()),
        Some(&rig.token),
    )
    .await;
    assert!(unknown.contains("\"reason\":\"missing\""), "{unknown}");
    teardown(rig);
}
