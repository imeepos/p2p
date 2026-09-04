//! 本地 WS 服务（需求 B，设计 §6 本地面）：绑 127.0.0.1 + 随机 token 双条件。
//! 鉴权在 HTTP 升级握手层完成（query ?token=），无 token/错 token 以 401 拒绝并留
//! 审计日志（防浏览器 drive-by）。每条 WS 连接对应一条到目标 peer 的 /dsh-acp/1 流。

use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use acp_common::consts::LINE_GUARD_LIMIT;
use http::StatusCode;
use p2p::{Node, PeerId};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use uuid::Uuid;

use crate::conn::{self, Authed};
use crate::state::StatusHub;
use crate::ticket::TicketStore;

/// 鉴权通过后的服务端依赖集。
#[derive(Clone)]
pub struct WsDeps {
    pub node: Arc<Node>,
    pub hub: Arc<StatusHub>,
    pub tickets: Arc<TicketStore>,
    pub window: Duration,
}

pub struct WsServer {
    pub addr: SocketAddr,
}

impl WsServer {
    /// 绑定并启动 accept 循环；addr 供 stdout ready 行发布。
    pub async fn start(port: u16, token: String, deps: WsDeps) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
        let addr = listener.local_addr()?;
        tokio::spawn(accept_loop(listener, token, deps));
        Ok(Self { addr })
    }
}

async fn accept_loop(listener: TcpListener, token: String, deps: WsDeps) {
    loop {
        match listener.accept().await {
            Ok((tcp, peer_addr)) => {
                if !peer_addr.ip().is_loopback() {
                    tracing::warn!(%peer_addr, "ws client from non-loopback rejected");
                    continue;
                }
                tokio::spawn(serve_conn(tcp, token.clone(), deps.clone()));
            }
            Err(err) => {
                tracing::warn!(error = %err, "ws accept failed");
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

// ErrorResponse（http::Response）体积较大：签名由 tungstenite Callback trait 固定，
// 本地无法收敛，豁免 result_large_err。
#[allow(clippy::result_large_err)]
async fn serve_conn(tcp: TcpStream, token: String, deps: WsDeps) {
    let (tx, rx) = std::sync::mpsc::sync_channel::<Result<Authed, String>>(1);
    let check = move |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
        match authorize(&parse_query(req.uri().query().unwrap_or("")), &token) {
            Ok(authed) => {
                let _ = tx.send(Ok(authed));
                Ok(resp)
            }
            Err(reason) => {
                let _ = tx.send(Err(reason.clone()));
                Err(unauthorized(reason))
            }
        }
    };
    let config = WebSocketConfig::default().max_message_size(Some(LINE_GUARD_LIMIT));
    let accepted = tokio_tungstenite::accept_hdr_async_with_config(tcp, check, Some(config)).await;
    let verdict = rx
        .try_recv()
        .unwrap_or_else(|_| Err("handshake failed before auth".into()));
    match (accepted, verdict) {
        (Ok(ws), Ok(authed)) => {
            conn::run_connection(deps.node, deps.hub, deps.tickets, ws, authed, deps.window).await;
        }
        (Ok(_), Err(reason)) => {
            tracing::error!(reason, "ws accepted despite auth rejection; dropping");
        }
        (Err(err), verdict) => {
            // 拒绝路径（无/错 token）与坏 HTTP 都落这里：reason 携带拒绝原因。
            let reason = verdict.err();
            tracing::warn!(error = %err, reason = ?reason, "ws handshake rejected or failed");
        }
    }
}

/// 鉴权 + 连接参数解析：token 精确匹配；peer 必填且必须可解析；reattach/atoken 可选。
fn authorize(params: &[(String, String)], token: &str) -> Result<Authed, String> {
    let given = param(params, "token").unwrap_or_default();
    if given.is_empty() {
        return Err("missing token".into());
    }
    if given != token {
        // 不记录 token 材质，只记长度，便于区分"没带"与"带错"。
        tracing::warn!(len = given.len(), "ws token mismatch");
        return Err("bad token".into());
    }
    let peer_raw = param(params, "peer")
        .filter(|s| !s.is_empty())
        .ok_or("missing peer")?;
    let peer: PeerId =
        crate::dial::parse_peer_id(peer_raw).map_err(|e| format!("bad peer: {e}"))?;
    let reattach = match param(params, "reattach") {
        Some(raw) if !raw.is_empty() => {
            Some(Uuid::parse_str(raw).map_err(|_| "bad reattach (want uuid)".to_string())?)
        }
        _ => None,
    };
    let agent_token = param(params, "atoken").map(str::to_string);
    Ok(Authed {
        peer,
        reattach,
        agent_token,
    })
}

fn param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// query 串解析：& 分隔、= 取值、%XX 与 + 解码；无 '=' 的键视为空值。
fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((k, v)) => (percent_decode(k), percent_decode(v)),
            None => (percent_decode(pair), String::new()),
        })
        .collect()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 3 <= bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3])
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(b) => {
                        out.push(b);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 构造 401 ErrorResponse：全部常量入参，无 panic 路径。
fn unauthorized(reason: String) -> ErrorResponse {
    let resp: ErrorResponse = http::Response::new(Some(format!(
        "{reason}
"
    )));
    let (mut parts, body) = resp.into_parts();
    parts.status = StatusCode::UNAUTHORIZED;
    http::Response::from_parts(parts, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(s: &str) -> Vec<(String, String)> {
        parse_query(s)
    }

    #[test]
    fn query_parses_pairs_and_empty_values() {
        assert_eq!(
            q("a=1&b=2"),
            vec![("a".into(), "1".into()), ("b".into(), "2".into())]
        );
        assert_eq!(q("flag"), vec![("flag".into(), String::new())]);
        assert_eq!(q(""), Vec::<(String, String)>::new());
    }

    #[test]
    fn percent_decode_handles_escapes_and_plus() {
        assert_eq!(percent_decode("a%20b+c"), "a b c");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("plain"), "plain");
    }

    #[test]
    fn authorize_rejects_missing_and_bad_token() {
        let err = authorize(&q("peer=x"), "secret").unwrap_err();
        assert_eq!(err, "missing token");
        let err = authorize(&q("token=wrong&peer=x"), "secret").unwrap_err();
        assert_eq!(err, "bad token");
    }

    #[test]
    fn authorize_requires_parsable_peer() {
        assert!(authorize(&q("token=t"), "t")
            .unwrap_err()
            .contains("missing peer"));
        assert!(authorize(&q("token=t&peer=!!"), "t")
            .unwrap_err()
            .contains("bad peer"));
    }

    #[test]
    fn unauthorized_is_401_with_reason_body() {
        let resp = unauthorized("missing token".into());
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(resp.body().as_deref(), Some("missing token\n"));
    }
}
