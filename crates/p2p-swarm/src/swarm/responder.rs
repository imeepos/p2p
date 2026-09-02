//! 打洞应答侧（design §7.3）：回 Ack + 探测请求方地址 + 接入其电路兜底。
//!
//! 请求方身份取信令 peer_id（relay 转发时已改写为真实发送方）。M4 防线延伸：
//! 电路握手后仍需与信令身份一致，不一致即丢弃。

use std::io;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use p2p_identity::PeerId;
use p2p_mux::{BoxedStream, YamuxMux, MAX_STREAMS_PER_CONN};
use p2p_relay::{CircuitId, PunchSession, RelayClient};
use p2p_security::{NoiseXx, SecurityUpgrade};
use p2p_transport::TransportAddr;

use super::dial::{dial_one, insert_connection};
use super::relay_session::PROBE_TIMEOUT;
use super::{Mux, Swarm};

/// 等待对端接入电路上限。
const JOIN_TIMEOUT: Duration = Duration::from_secs(10);
/// 探测单地址上限。
/// 电路接入项前缀（与 relay_session 的发起侧约定一致）。
const CID_PREFIX: &str = "cid/";

/// 入站 PunchReq：回 Ack（宣告本端地址）→ 探测请求方地址 → 失败则接入其电路。
pub(super) async fn handle_punch_req(
    swarm: &Swarm,
    client: &mut RelayClient,
    req: p2p_relay::PunchReq,
) {
    let Some(peer) = parse_peer_id(&req.peer_id) else {
        tracing::warn!(peer = %req.peer_id, "punch req from unparseable peer id; ignored");
        return;
    };
    let mut session = PunchSession::responder(&req);
    let ack = session.build_ack(swarm.punch_addrs_strs());
    if let Err(e) = client.reply_punch(ack).await {
        tracing::warn!(%peer, error = %e, "punch ack reply failed");
        return;
    }
    let _ = session.mark_probing();

    let (probe_addrs, cid) = split_offer(&req.addrs);
    let mut last = "no probe addrs".to_string();
    for addr in probe_addrs {
        let attempt = tokio::time::timeout(PROBE_TIMEOUT, dial_one(swarm, peer, &addr));
        match attempt.await {
            Ok(Ok(mux)) => {
                tracing::info!(%peer, %addr, "inbound punch probe landed direct connection");
                insert_connection(swarm, peer, mux);
                return;
            }
            Ok(Err(err)) => last = err.to_string(),
            Err(_) => last = "probe timeout".to_string(),
        }
    }
    let Some(cid) = cid else {
        tracing::warn!(%peer, last = %last, "punch probes failed and no circuit offered");
        return;
    };
    let stream = match tokio::time::timeout(JOIN_TIMEOUT, client.connect(CircuitId(cid))).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            tracing::warn!(%peer, circuit = cid, error = %e, "circuit join failed");
            return;
        }
        Err(_) => {
            tracing::warn!(%peer, circuit = cid, "circuit join timed out");
            return;
        }
    };
    let (remote, mux) = match secure_inbound(swarm, stream).await {
        Ok(pair) => pair,
        Err(e) => {
            tracing::warn!(%peer, error = %e, "circuit handshake failed");
            return;
        }
    };
    if remote != peer {
        tracing::warn!(peer = %remote, expected = %peer, "circuit identity mismatch; dropping");
        return;
    }
    if !swarm.gate_allows(remote).await {
        tracing::warn!(%peer, "circuit connection denied by gate");
        return;
    }
    tracing::info!(%peer, circuit = cid, "circuit connection established (inbound)");
    insert_connection(swarm, remote, mux);
}

/// 电路流入站安全升级：Noise XX（被动侧）+ yamux。
async fn secure_inbound(swarm: &Swarm, stream: BoxedStream) -> io::Result<(PeerId, Mux)> {
    let (remote, enc) = NoiseXx::new()
        .inbound(stream, &swarm.keypair)
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;
    let mux = Arc::new(YamuxMux::new(enc, false, MAX_STREAMS_PER_CONN));
    Ok((remote, mux))
}

/// 拆分信令地址：cid/N 为电路接入项，其余为探测地址。
pub(super) fn split_offer(addrs: &[String]) -> (Vec<TransportAddr>, Option<u64>) {
    let mut probes = Vec::new();
    let mut cid = None;
    for s in addrs {
        if let Some(rest) = s.strip_prefix(CID_PREFIX) {
            match rest.parse::<u64>() {
                Ok(n) => cid = Some(n),
                Err(_) => tracing::warn!(entry = %s, "malformed circuit entry ignored"),
            }
        } else if let Some(addr) = parse_probe_addr(s) {
            probes.push(addr);
        } else {
            tracing::debug!(entry = %s, "unparseable probe addr ignored");
        }
    }
    (probes, cid)
}

/// 解析探测地址：ip/u端口、ip/t端口，兼容裸 ip:端口（按 TCP）。
pub(super) fn parse_probe_addr(s: &str) -> Option<TransportAddr> {
    if let Some((ip_str, tail)) = s.split_once('/') {
        let ip: IpAddr = ip_str.parse().ok()?;
        let mut rest = tail.chars();
        let kind = rest.next()?;
        let port: u16 = rest.as_str().parse().ok()?;
        return match kind {
            'u' => Some(TransportAddr::Quic { ip, port }),
            't' => Some(TransportAddr::Tcp { ip, port }),
            _ => None,
        };
    }
    let addr = s.parse::<std::net::SocketAddr>().ok()?;
    Some(TransportAddr::Tcp {
        ip: addr.ip(),
        port: addr.port(),
    })
}

fn parse_peer_id(s: &str) -> Option<PeerId> {
    let bytes: [u8; 32] = bs58::decode(s).into_vec().ok()?.try_into().ok()?;
    Some(PeerId::from_bytes(bytes))
}
