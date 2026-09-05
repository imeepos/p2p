//! 单条 WS 连接的编排（需求 A/B/C 汇合）：拨号 → 握手 → 在线泵送 →
//! 断流进续连窗口（窗口内票据在盘，ACP4 续连）→ 到期 offline。
//! 全部失败路径留日志 + 状态迁移，禁止静默。

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use p2p::{Node, NodeEvent, PeerId};
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message};
use tokio_tungstenite::WebSocketStream;
use uuid::Uuid;

use crate::dial::{self, DialError, HandshakeOutcome};
use crate::pump::{self, PumpEnd};
use crate::state::{ConnPhase, StatusHub};
use crate::ticket::{ReattachTicket, TicketStore};

/// WS 握手鉴权通过后的连接参数（ws.rs 解析产物）。
#[derive(Debug)]
pub struct Authed {
    pub peer: PeerId,
    pub reattach: Option<Uuid>,
    pub agent_token: Option<String>,
}

/// WS Close 自定义码：44xx 段，GUI 波按码区分失败面。
const CLOSE_DENIED: u16 = 4403;
const CLOSE_DIAL_FAILED: u16 = 4500;

pub async fn run_connection(
    node: Arc<Node>,
    hub: Arc<StatusHub>,
    tickets: Arc<TicketStore>,
    ws: WebSocketStream<TcpStream>,
    authed: Authed,
    window: Duration,
) {
    let peer = authed.peer.to_string();
    hub.transition(ConnPhase::Connecting, Some(peer.clone()), None, None);
    match dial::dial_and_handshake(&node, authed.peer, authed.agent_token, authed.reattach).await {
        Ok((_, outcome, stream)) => {
            let conn = outcome.conn;
            persist_ticket(&tickets, &peer, &outcome);
            hub.transition(
                ConnPhase::Online,
                Some(peer.clone()),
                Some(conn.to_string()),
                None,
            );
            let (sink, inbound) = ws.split();
            let mut events = node.events();
            // 泵与对端断连事件竞速：底座半关闭无原语，agent 死亡的可观察
            // 信号是 PeerDisconnected（设计 §7 生命周期矩阵），必须并路等待。
            tokio::select! {
                end = pump::run(stream, sink, inbound) => {
                    tracing::info!(peer = %peer, end = ?end, "connection pump finished");
                    settle_after_loss(
                        &hub,
                        &tickets,
                        peer,
                        conn,
                        window,
                        Some(end_reason(end)),
                    )
                    .await;
                }
                () = wait_peer_disconnect(&mut events, authed.peer) => {
                    tracing::warn!(peer = %peer, "peer connection lost; ending pump");
                    settle_after_loss(
                        &hub,
                        &tickets,
                        peer,
                        conn,
                        window,
                        Some("peer connection lost".into()),
                    )
                    .await;
                }
            }
        }
        Err(DialError::Denied(code)) => {
            tracing::warn!(peer = %peer, code = %code, "agent denied handshake");
            hub.transition(
                ConnPhase::Offline,
                Some(peer.clone()),
                None,
                Some(format!("denied: {code}")),
            );
            close_ws(ws, CLOSE_DENIED, &format!("denied: {code}")).await;
        }
        Err(err) => {
            tracing::warn!(peer = %peer, error = %err, "dial or handshake failed");
            hub.transition(
                ConnPhase::Offline,
                Some(peer.clone()),
                None,
                Some(err.to_string()),
            );
            close_ws(ws, CLOSE_DIAL_FAILED, "dial-failed").await;
        }
    }
}

/// 断流（泵结束或对端断连）后进入续连窗口；到期且期间未被新连接接管才落 offline。
/// 窗口起点同步登记进票据（status /reattach 可用性判定的锚点）。
async fn settle_after_loss(
    hub: &StatusHub,
    tickets: &TicketStore,
    peer: String,
    conn: Uuid,
    window: Duration,
    detail: Option<String>,
) {
    let lost_at_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    if let Err(err) = tickets.mark_lost(&peer, lost_at_unix_ms) {
        tracing::error!(peer, %conn, error = %err, "reattach ticket mark_lost failed");
    }
    hub.transition(
        ConnPhase::ReattachWindow,
        Some(peer.clone()),
        Some(conn.to_string()),
        detail,
    );
    tokio::time::sleep(window).await;
    let snap = hub.snapshot();
    if snap.phase == ConnPhase::ReattachWindow && snap.peer.as_deref() == Some(peer.as_str()) {
        hub.transition(
            ConnPhase::Offline,
            Some(peer),
            Some(conn.to_string()),
            Some("reattach window expired".into()),
        );
    }
}

/// 等待目标 peer 的断连事件；通道关闭（节点关停）同视为断连。
async fn wait_peer_disconnect(events: &mut broadcast::Receiver<NodeEvent>, peer: PeerId) {
    loop {
        match events.recv().await {
            Ok(NodeEvent::PeerDisconnected { peer: lost }) if lost == peer => return,
            Ok(_) => {}
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                tracing::warn!(skipped, "node events lagged while watching disconnect");
            }
            Err(broadcast::error::RecvError::Closed) => return,
        }
    }
}

fn end_reason(end: PumpEnd) -> String {
    match end {
        PumpEnd::PeerClosed => "peer stream closed".into(),
        PumpEnd::ClientClosed => "client ws closed".into(),
        PumpEnd::Failed => "pump failed".into(),
    }
}

/// 连接成功即落票据（桥签发票据 + conn + 目标 PeerId，ACP4 续连入口）。
/// 落盘失败不断连接，但必须留 error 日志（可观测信号）。
fn persist_ticket(tickets: &TicketStore, peer: &str, outcome: &HandshakeOutcome) {
    let saved_at_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let ticket = ReattachTicket::new(outcome.conn, peer, saved_at_unix_ms, outcome.ticket.clone());
    match tickets.save(ticket) {
        Ok(()) => tracing::info!(peer, conn = %outcome.conn, "reattach ticket saved"),
        Err(err) => tracing::error!(
            peer,
            conn = %outcome.conn,
            error = %err,
            "reattach ticket save failed"
        ),
    }
}

async fn close_ws(ws: WebSocketStream<TcpStream>, code: u16, reason: &str) {
    let (mut sink, _inbound) = ws.split();
    let frame = CloseFrame {
        code: CloseCode::from(code),
        reason: reason.into(),
    };
    let _ = sink.send(Message::Close(Some(frame))).await;
    let _ = sink.close().await;
}
