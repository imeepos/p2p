//! 本地 status 端点（查询面，GUI 波依赖；README 为契约权威）：
//! GET /status 返回连接状态机快照，GET /discovery 返回发现候选清单，
//! GET /reattach?peer= 返回该 peer 当前可用的续连票据（窗口内，不过期不返）；
//! Bearer token 鉴权，绑 127.0.0.1。手写最小 HTTP/1.1 头解析，不引服务端框架。

use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::net::{TcpListener, TcpStream};

use crate::discovery::DiscoveryHub;
use crate::share;
use crate::state::{now_unix_ms, StatusHub};
use crate::status_wire::{drain_exact, header_value, parse_head, read_body, read_request, reply};
use crate::ticket::{TicketQuery, TicketStore};

#[derive(Clone)]
pub struct StatusDeps {
    pub hub: Arc<StatusHub>,
    pub discovery: Arc<DiscoveryHub>,
    pub tickets: Arc<TicketStore>,
    pub window: Duration,
    /// P2P 节点（/connect-share 直拨复用本进程拨号面）。
    pub node: Arc<p2p::Node>,
    /// 本地 WS 服务地址（/connect-share 经其复用连接编排）。
    pub ws_addr: SocketAddr,
    /// 本地 WS 鉴权 token（与 status 同源，connect_via_link 自连用）。
    pub ws_token: String,
}

pub struct StatusServer {
    pub addr: SocketAddr,
}

impl StatusServer {
    pub async fn start(port: u16, token: String, deps: StatusDeps) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
        let addr = listener.local_addr()?;
        tokio::spawn(accept_loop(listener, token, deps));
        Ok(Self { addr })
    }
}

async fn accept_loop(listener: TcpListener, token: String, deps: StatusDeps) {
    loop {
        match listener.accept().await {
            Ok((tcp, peer_addr)) => {
                if !peer_addr.ip().is_loopback() {
                    tracing::warn!(%peer_addr, "status client from non-loopback rejected");
                    continue;
                }
                tokio::spawn(serve_conn(tcp, token.clone(), deps.clone()));
            }
            Err(err) => {
                tracing::warn!(error = %err, "status accept failed");
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn serve_conn(mut tcp: TcpStream, token: String, deps: StatusDeps) {
    let (head, body_prefix) = match read_request(&mut tcp).await {
        Ok((head, body_prefix)) => (head, body_prefix),
        Err(err) => {
            tracing::warn!(error = %err, "status: bad request head");
            return;
        }
    };
    let want = header_value(&head, "content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let mut consumed = body_prefix.len().min(want);
    let (method, target, bearer) = parse_head(&head);
    if bearer.as_deref() != Some(token.as_str()) {
        tracing::warn!(%target, "status: unauthorized request");
        reply(
            &mut tcp,
            401,
            "Unauthorized",
            "{\"error\":\"unauthorized\"}",
        )
        .await;
        drain_exact(&mut tcp, want.saturating_sub(consumed)).await;
        return;
    }
    let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));
    match (method.as_str(), path) {
        ("GET", "/status") => {
            let body = serde_json::to_string(&deps.hub.snapshot())
                .unwrap_or_else(|_| "{\"error\":\"serialize\"}".into());
            reply(&mut tcp, 200, "OK", &body).await;
        }
        ("GET", "/discovery") => {
            let snapshot = deps.discovery.snapshot();
            let body = serde_json::to_string(&Value::Object(
                [(
                    "peers".to_string(),
                    serde_json::to_value(snapshot).unwrap_or(Value::Null),
                )]
                .into_iter()
                .collect(),
            ))
            .unwrap_or_else(|_| "{\"error\":\"serialize\"}".into());
            reply(&mut tcp, 200, "OK", &body).await;
        }
        ("GET", "/reattach") => handle_reattach(&mut tcp, &deps, query).await,
        ("POST", "/connect-share") => match read_body(&mut tcp, &head, body_prefix).await {
            Ok(body) => {
                consumed = want;
                share::handle_connect_share(&mut tcp, &deps, &body).await;
            }
            Err(err) => {
                tracing::warn!(error = %err, "connect-share: unreadable body");
                reply(
                    &mut tcp,
                    400,
                    "Bad Request",
                    "{\"error\":\"bad-request\",\"reason\":\"unreadable body\"}",
                )
                .await;
            }
        },
        _ => reply(&mut tcp, 404, "Not Found", "{\"error\":\"not-found\"}").await,
    }
    drain_exact(&mut tcp, want.saturating_sub(consumed)).await;
}

/// GET /reattach?peer=<base58>：该 peer 当前可用的续连票据。
/// 不存在（missing）/已过期（expired）/存储不可读（unavailable）如实反映，
/// 过期票据绝不返回（README 契约）。
async fn handle_reattach(tcp: &mut TcpStream, deps: &StatusDeps, query: &str) {
    let Some(peer) = query_param(query, "peer").filter(|p| !p.is_empty()) else {
        reply(tcp, 400, "Bad Request", "{\"error\":\"missing-peer\"}").await;
        return;
    };
    let body = match deps.tickets.usable_for(peer, deps.window, now_unix_ms()) {
        Ok(q) => ticket_body(peer, &q),
        Err(err) => {
            tracing::error!(peer, error = %err, "reattach query failed");
            ticket_body(peer, &TicketQuery::Missing)
        }
    };
    reply(tcp, 200, "OK", &body).await;
}

fn ticket_body(peer: &str, query: &TicketQuery) -> String {
    let (ticket, expires, reason) = match query {
        TicketQuery::Usable(t) => (Some(t.ticket.as_str()), Some(t.expires_at_unix_ms), "ok"),
        TicketQuery::Missing => (None, None, "missing"),
        TicketQuery::Expired => (None, None, "expired"),
    };
    serde_json::json!({
        "peer": peer,
        "ticket": ticket,
        "expires_at_unix_ms": expires,
        "reason": reason,
    })
    .to_string()
}

/// 取查询串参数（base58 peer 无需百分号解码，缺参为 None）。
fn query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then_some(v)
    })
}
