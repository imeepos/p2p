//! 出站路径与降级链（design §7.3/§8）：直连按地址簿顺序尝试，
//! 失败进入 打洞 → 中继电路 兜底；每一跳结果发 DialHop 事件（§12）。

use std::io;
use std::sync::Arc;
use std::time::Duration;

use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::{dispatch_inbound, ProtocolError};
use p2p_transport::{Transport, TransportAddr};
use tokio::sync::{broadcast, watch};

use super::lifecycle::LifecycleMsg;
use super::{Mux, RegistryCell, Swarm};
use crate::pool::{Admission, ConnectionPool};
use crate::{DialHop, NodeEvent};

/// 连接方向：本端主动拨出为 Outbound，对端拨入为 Inbound。
/// 收敛裁决据此让两端对同一条连接得出一致结论（见 converge_prefers_new）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ConnDirection {
    Outbound,
    Inbound,
}

/// hairpin 候选地址的拨号预算（E4）：同 NAT 回环路径要么毫秒级通、要么不通，
/// 不值得占满传输层默认超时（TCP 连接 5s / QUIC 握手 10s），快速失败让位后续地址。
const HAIRPIN_DIAL_TIMEOUT: Duration = Duration::from_secs(2);

/// 按地址簿顺序逐一尝试直连；每个失败地址发 DialFailed，全部失败返回末次错误。
/// hairpin 候选（与自身观测地址同公网前缀）施加短超时，保证 LAN 地址轮得到。
pub(super) async fn dial_peer(swarm: &Swarm, peer: PeerId) -> io::Result<Mux> {
    if peer == swarm.local_peer_id() {
        return Err(dial_rejected(swarm, peer, "refusing to dial self"));
    }
    let addrs = swarm.addresses_of(peer);
    if addrs.is_empty() {
        return Err(dial_rejected(swarm, peer, "no known address for peer"));
    }
    let mut tried = 0usize;
    let mut last_err: Option<io::Error> = None;
    for (addr, hairpin) in addrs {
        tried += 1;
        match dial_limited(swarm, peer, &addr, hairpin).await {
            Ok(mux) => {
                swarm.metrics.hop_ok(DialHop::Direct);
                insert_connection(swarm, peer, mux.clone(), ConnDirection::Outbound);
                return Ok(mux);
            }
            Err(err) => {
                // 每个失败地址都留 info 日志 + DialFailed 事件，生产默认级别即可归因
                tracing::info!(%peer, %addr, error = %err, "direct addr failed; trying next");
                swarm.metrics.count_addr_dial_fail();
                swarm.emit(NodeEvent::DialFailed {
                    peer: Some(peer),
                    reason: format!("{addr}: {err}"),
                });
                last_err = Some(err);
            }
        }
    }
    swarm.metrics.hop_fail(DialHop::Direct);
    swarm.emit(NodeEvent::DialHop {
        peer,
        hop: DialHop::Direct,
        ok: false,
        detail: format!("{tried} addr(s) tried"),
    });
    let direct_reason = last_err
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "no addr attempted".to_string());
    // 降级链 2/3 跳：打洞 + 中继电路（未配置 relay 则止于直连，已留 debug 日志）
    if swarm.has_relay_sessions() {
        match swarm.relay_degrade(peer).await {
            Ok(mux) => {
                insert_connection(swarm, peer, mux.clone(), ConnDirection::Outbound);
                return Ok(mux);
            }
            Err(err) => {
                swarm.metrics.hop_fail(DialHop::Relay);
                swarm.emit(NodeEvent::DialHop {
                    peer,
                    hop: DialHop::Relay,
                    ok: false,
                    detail: err.to_string(),
                });
                // 完整原因链（E5）：同步调用方拿到的末次错误携带全部跳失败原因
                return Err(io::Error::other(format!(
                    "all hops failed for {peer}; direct: {direct_reason}; relay: {err}"
                )));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| io::Error::other("dial failed")))
}

fn dial_rejected(swarm: &Swarm, peer: PeerId, reason: &str) -> io::Error {
    swarm.emit(NodeEvent::DialFailed {
        peer: Some(peer),
        reason: reason.to_string(),
    });
    io::Error::other(reason)
}

/// hairpin 候选加短超时（E4）：refused/黑洞不得吃满单地址请求预算。
async fn dial_limited(
    swarm: &Swarm,
    peer: PeerId,
    addr: &TransportAddr,
    hairpin: bool,
) -> io::Result<Mux> {
    if !hairpin {
        return dial_one(swarm, peer, addr).await;
    }
    tokio::time::timeout(HAIRPIN_DIAL_TIMEOUT, dial_one(swarm, peer, addr))
        .await
        .unwrap_or_else(|_| {
            Err(io::Error::other(format!(
                "hairpin dial timeout after {HAIRPIN_DIAL_TIMEOUT:?}"
            )))
        })
}

/// 单地址拨号 + 门禁裁决；不放行即断链（conn 随作用域丢弃而关闭）。
///
/// 安全不变式（security-review-1.md M4）：本函数是 swarm 唯一拨号出口，
/// expected 恒为 Some(peer)——地址簿可被投毒，握手后的身份比对是最后防线，
/// 类型层面 peer 必填，调用方无法省略。盲拨仅存在于 facade rendezvous
/// 链接（bootstrap 身份未知）且必须留日志。
pub(super) async fn dial_one(swarm: &Swarm, peer: PeerId, addr: &TransportAddr) -> io::Result<Mux> {
    let transport: &dyn Transport = match addr {
        TransportAddr::Quic { .. } => &swarm.dial_quic,
        TransportAddr::Tcp { .. } => &swarm.dial_tcp,
    };
    let conn = transport
        .dial(addr, &swarm.keypair, Some(peer))
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;
    if !swarm.gate_allows(conn.remote).await {
        tracing::warn!(peer = %conn.remote, %addr, "outbound connection denied by gate, dropping");
        swarm.metrics.count_gate_denial();
        drop(conn);
        return Err(io::Error::other(format!(
            "peer {peer} denied by connection gate"
        )));
    }
    Ok(conn.mux)
}

/// 幂等入池：按收敛规则入池或顶替，胜者发 PeerConnected 并启动收流分发；
/// 落选连接显式 close（静默 drop 会遗留 yamux 驱动任务，对端还以为连着）。
///
/// 双向同时拨号竞态（两端各拨一次产生两条连接）的收敛规则：
/// 恒保留「较小 PeerId 一端拨出的那条」。本端只需方向 + 本地/对端 id 即可本地裁决，
/// 两端对每条连接的方向认知相反、结论一致，最终两点各持同一条连接。
/// 若无此收敛，各留各的会出现：A 池里是 A 拨的、B 池里是 B 拨的，
/// 流与 serve 循环分家，单方向 request 永远无应答（2026-09 GUI 闪断实测根因）。
pub(super) fn insert_connection(swarm: &Swarm, peer: PeerId, mux: Mux, direction: ConnDirection) {
    let prefer_new = converge_prefers_new(swarm, peer, direction);
    match swarm.pool.admit(peer, mux.clone(), prefer_new) {
        Admission::RejectedExisting(dup) => {
            tracing::debug!(%peer, "duplicate connection converged to existing, closing new");
            dup.close();
        }
        Admission::Accepted | Admission::Replaced(_) => {
            swarm.emit(NodeEvent::PeerConnected { peer });
            // E6 钩子：入池即连接事实（入站/出站共用唯一入口），交监督者裁决转移
            swarm.lifecycle.notify(LifecycleMsg::Connected { peer });
            let ctx = ServeCtx {
                pool: swarm.pool.clone(),
                registry: swarm.registry.clone(),
                events: swarm.events.clone(),
                shutdown: swarm.shutdown_rx.clone(),
                lifecycle: swarm.lifecycle.clone(),
            };
            tokio::spawn(serve_connection(ctx, peer, mux));
        }
    }
}

/// 新连接是否胜出：local < remote 时胜者是 Inbound（小 id 一端拨出的），
/// local > remote 时胜者是 Outbound。与本端第几条到达无关，只看方向。
fn converge_prefers_new(swarm: &Swarm, peer: PeerId, direction: ConnDirection) -> bool {
    let keep_outbound = swarm.local_peer_id() > peer;
    (direction == ConnDirection::Outbound) == keep_outbound
}

/// serve 任务的独立组件集：不持有 Swarm 本体，生命周期与连接一致。
struct ServeCtx {
    pool: Arc<ConnectionPool>,
    registry: RegistryCell,
    events: broadcast::Sender<NodeEvent>,
    shutdown: watch::Receiver<bool>,
    /// E6：断链回报监督者（Connected→BackingOff 并排定重连）。
    lifecycle: super::lifecycle::LifecycleHandle,
}

/// 收流分发循环：连接关闭或关停即出池并发 PeerDisconnected（断开路径可见）。
async fn serve_connection(ctx: ServeCtx, peer: PeerId, mux: Mux) {
    let mut shutdown = ctx.shutdown;
    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            stream = mux.accept_stream() => match stream {
                Some(stream) => {
                    tokio::spawn(dispatch_stream(ctx.registry.clone(), ctx.events.clone(), peer, stream));
                }
                None => break,
            },
        }
    }
    // 仅当本连接仍在册才发断开事件：被顶替的旧连接退出时池里已是新连接，
    // 此刻发 PeerDisconnected 是谎报（GUI 会把活连接渲染成断开）。
    if ctx.pool.remove_if_same(&peer, &mux) {
        let _ = ctx.events.send(NodeEvent::PeerDisconnected { peer });
        // E6 钩子：本连接确已出池（被顶替的旧连接不进来，不谎报断链）
        ctx.lifecycle.notify(LifecycleMsg::LinkLost { peer });
    }
}

/// 单条入站流分发：协议违规（含未注册协议）发事件；纯 io 关闭只留调试日志。
async fn dispatch_stream(
    registry: RegistryCell,
    events: broadcast::Sender<NodeEvent>,
    peer: PeerId,
    stream: BoxedStream,
) {
    let snapshot = registry.lock().expect("registry lock").clone();
    match dispatch_inbound(stream, &snapshot).await {
        Ok(()) => {}
        Err(ProtocolError::Io(err)) => {
            tracing::debug!(%peer, error = %err, "inbound stream closed");
        }
        Err(other) => {
            let reason = other.to_string();
            tracing::warn!(%peer, %reason, "protocol violation on inbound stream");
            let _ = events.send(NodeEvent::ProtocolViolation { peer, reason });
        }
    }
}
