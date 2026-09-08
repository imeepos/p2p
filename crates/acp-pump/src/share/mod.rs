//! 分享链接直拨编排（docs/design/acp-share-design.md §7）：
//! 解析 → 登记 peer 地址候选（沿 --peer 同机制）→ 拨号 → 握手 token=链接 token。
//! 拨号经本地 WS 通道复用既有连接编排（conn.rs）：denied/dial-fail 路径、
//! 状态机、reattach 票据照常生效。观察到 ready 即回执并收拢激活连接
//! （首连激活后，该 peer 此后凭 agent 侧策略表如普通 endpoint 连接，重复导入幂等）。
//! 结果进 stdout JSON 行 + /status 状态面 + /connect-share 响应体，禁止静默。

mod link;

pub use link::{parse_share_link, ShareLink, ShareLinkError, SCHEME, TOKEN_HEX_LEN, VERSION};

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use p2p::Node;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::discovery::{self, DiscoveryHub};
use crate::out;
use crate::state::{now_unix_ms, ConnPhase, StatusHub};
use crate::status::StatusDeps;
use crate::status_wire::reply;

/// 拨号→握手结果等待上限：握手护栏 10s（dial::HANDSHAKE_TIMEOUT）+ 余量。
const CONNECT_WAIT: Duration = Duration::from_secs(15);

/// connect-share 结果：stdout 事件行、HTTP 响应体与 /status detail 同一词汇。
#[derive(Clone, Debug, serde::Serialize)]
pub struct ShareOutcome {
    pub peer: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl ShareOutcome {
    fn fail(peer: &str, reason: &str) -> Self {
        Self {
            peer: peer.to_string(),
            ok: false,
            conn: None,
            reason: Some(reason.to_string()),
        }
    }
}

/// 完整直拨流程。所有路径回传结果并打 stdout 事件；token 原文不进任何日志。
pub async fn connect_via_link(
    node: Arc<Node>,
    hub: Arc<StatusHub>,
    disc: Arc<DiscoveryHub>,
    ws_addr: SocketAddr,
    ws_token: String,
    link: &ShareLink,
) -> ShareOutcome {
    discovery::register_peer(&node, &disc, &link.peer, &link.addrs, "share").await;
    // 握手 token = 链接 token，经 WS 查询参数 atoken 逐连接透传（ws.rs 契约）。
    let url = format!(
        "ws://127.0.0.1:{}/?token={}&peer={}&atoken={}",
        ws_addr.port(),
        ws_token,
        link.peer,
        link.token
    );
    let ws = match tokio_tungstenite::connect_async(url).await {
        Ok((ws, _)) => Some(ws),
        Err(err) => {
            tracing::warn!(peer = %link.peer, error = %err, "share link: local ws connect failed");
            None
        }
    };
    let outcome = match ws.as_ref() {
        None => ShareOutcome::fail(&link.peer, "local ws connect failed"),
        Some(_) => match wait_verdict(&hub).await {
            Verdict::Ready(conn) => ShareOutcome {
                peer: link.peer.clone(),
                ok: true,
                conn,
                reason: None,
            },
            Verdict::Failed(reason) => {
                tracing::warn!(peer = %link.peer, %reason, "share link connect failed");
                ShareOutcome::fail(&link.peer, &reason)
            }
            Verdict::Timeout => {
                tracing::warn!(peer = %link.peer, wait = ?CONNECT_WAIT, "share link connect timed out");
                ShareOutcome::fail(&link.peer, "connect-timeout")
            }
        },
    };
    // 激活连接为一次性：观察到结果即收拢本地 WS，连接按普通断链路径进
    // reattach 窗口收尾；此后该 peer 凭策略表正常连接（§7 幂等语义）。
    if let Some(ws) = ws {
        let (mut sink, _) = ws.split();
        if let Err(err) = sink.send(Message::Close(None)).await {
            tracing::debug!(peer = %link.peer, error = %err, "share link: ws close failed");
        }
    }
    out::event("share-connect", &outcome);
    outcome
}

enum Verdict {
    Ready(Option<String>),
    Failed(String),
    Timeout,
}

/// 等待本次握手结果。判定以时间锚点为准：连接发起时刻之后的迁移才属于本次
/// 尝试——watch 只保留最新值，拨号秒失败时 Connecting→Offline 会被合并成一次
/// 变化，仅凭「观察到过 Connecting」会把真实失败误判为陈旧快照。
async fn wait_verdict(hub: &StatusHub) -> Verdict {
    let started_at = now_unix_ms();
    let mut rx = hub.subscribe();
    let deadline = tokio::time::Instant::now() + CONNECT_WAIT;
    loop {
        let snap = rx.borrow().clone();
        if snap.since_unix_ms >= started_at {
            match snap.phase {
                ConnPhase::Connecting => {}
                ConnPhase::Online => return Verdict::Ready(snap.conn),
                ConnPhase::Offline => {
                    return Verdict::Failed(
                        snap.detail.unwrap_or_else(|| "dial failed".to_string()),
                    )
                }
                ConnPhase::ReattachWindow => {}
            }
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Verdict::Timeout;
        }
        match tokio::time::timeout(deadline - now, rx.changed()).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return Verdict::Failed("status hub closed".to_string()),
            Err(_) => return Verdict::Timeout,
        }
    }
}

/// POST /connect-share {"link": "..."}：成功与拨号/握手失败均 200 回 JSON 结果
/// （失败 reason 携带 denied 码）；坏入参 400；Bearer 鉴权由 status 服务统一前置。
pub async fn handle_connect_share(tcp: &mut TcpStream, deps: &StatusDeps, body: &str) {
    let link_raw = match parse_request_link(body) {
        Ok(link) => link,
        Err(reason) => {
            tracing::warn!(%reason, "connect-share: bad request body");
            let payload = serde_json::json!({"error": "bad-request", "reason": reason});
            reply(tcp, 400, "Bad Request", &payload.to_string()).await;
            return;
        }
    };
    let link = match parse_share_link(&link_raw) {
        Ok(link) => link,
        Err(err) => {
            tracing::warn!(error = %err, "connect-share: bad link");
            let payload = serde_json::json!({"error": "bad-link", "reason": err.to_string()});
            reply(tcp, 400, "Bad Request", &payload.to_string()).await;
            return;
        }
    };
    let outcome = connect_via_link(
        deps.node.clone(),
        deps.hub.clone(),
        deps.discovery.clone(),
        deps.ws_addr,
        deps.ws_token.clone(),
        &link,
    )
    .await;
    let payload =
        serde_json::to_string(&outcome).unwrap_or_else(|_| "{\"error\":\"serialize\"}".to_string());
    reply(tcp, 200, "OK", &payload).await;
}

/// 请求体 → 链接原文。失败文案固定措辞：入参可能含 token 原文，不进日志与响应。
fn parse_request_link(body: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|_| "invalid json body".to_string())?;
    value
        .get("link")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| "missing link".to_string())
}
