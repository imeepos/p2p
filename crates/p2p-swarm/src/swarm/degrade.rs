//! 降级链 2/3 跳（design §7.3，自 relay_session 拆出）：预留电路、打洞信令
//! 有界重试、探测对端地址、失败接入选定电路兜底与出站安全升级。
//! 每一跳结果发 DialHop 事件，禁止静默降级（design §12）。

use std::io;
use std::sync::Arc;
use std::time::Duration;

use p2p_identity::PeerId;
use p2p_mux::{BoxedStream, YamuxMux, MAX_STREAMS_PER_CONN};
use p2p_relay::{CircuitId, PunchAck, PunchSession, RelayClient};
use p2p_security::{NoiseXx, SecurityUpgrade};

use super::dial::dial_one;
use super::responder::parse_probe_addr;
use super::{Mux, Swarm};
use crate::error_chain::chained;
use crate::{DialHop, NodeEvent};

/// 电路预留 TTL；对端未在期内接入即由 relay 回收。
const CIRCUIT_TTL: Duration = Duration::from_secs(120);
/// 等待对端接入电路上限。
const JOIN_TIMEOUT: Duration = Duration::from_secs(10);
/// 探测单地址上限（应答侧共用）。
pub(super) const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// 打洞信令有界重试：对端会话可能仍在重连窗口内。
const SIGNAL_RETRIES: u32 = 5;
/// 信令重试间隔。
const SIGNAL_RETRY_GAP: Duration = Duration::from_millis(400);
/// 电路接入项前缀。
pub(super) const CID_PREFIX: &str = "cid/";

/// 降级链 2/3 跳：预留电路并随信令通告，探测对端地址，失败接入选定电路。
pub(super) async fn degrade(
    swarm: &Swarm,
    client: &mut RelayClient,
    peer: PeerId,
) -> io::Result<Mux> {
    let cid = reserve_circuit(swarm, client, peer).await?;
    let mut offer = swarm.punch_addrs_strs();
    offer.push(format!("{CID_PREFIX}{}", cid.0));

    let mut session = PunchSession::initiator(peer.to_string());
    let ack = signal_with_retries(swarm, client, peer, &offer).await?;
    stage_request_sent(&mut session)?;
    stage_on_ack(&mut session)?;

    if let Some(mux) = probe_for_direct(swarm, peer, &ack.addrs).await? {
        return Ok(mux);
    }
    emit_punch_fail(
        swarm,
        peer,
        "probes failed; falling back to relay circuit".to_string(),
    );

    // 中继兜底：接入电路（对端按信令接入即配对），密文流上完成安全握手
    let stream = join_circuit(client, cid.0).await?;
    let mux = secure_outbound(swarm, stream, peer).await?;
    swarm.metrics.hop_ok(DialHop::Relay);
    swarm.emit(NodeEvent::DialHop {
        peer,
        hop: DialHop::Relay,
        ok: true,
        detail: "circuit established".to_string(),
    });
    Ok(mux)
}

/// 预留发起侧电路；失败发 Punch 失败事件后原样上抛。
async fn reserve_circuit(
    swarm: &Swarm,
    client: &mut RelayClient,
    peer: PeerId,
) -> io::Result<CircuitId> {
    client
        .reserve(CIRCUIT_TTL, &peer.to_string())
        .await
        .map_err(|e| {
            emit_punch_fail(swarm, peer, format!("relay reserve: {e}"));
            chained(e)
        })
}

/// 打洞信令有界重试：对端控制链路可能尚未在 relay 上就绪。
async fn signal_with_retries(
    swarm: &Swarm,
    client: &mut RelayClient,
    peer: PeerId,
    offer: &[String],
) -> io::Result<PunchAck> {
    let mut ack = None;
    for attempt in 1..=SIGNAL_RETRIES {
        match client
            .request_punch(&peer.to_string(), offer.to_vec())
            .await
        {
            Ok(a) => {
                ack = Some(a);
                break;
            }
            Err(e) if attempt < SIGNAL_RETRIES => {
                tracing::debug!(%peer, attempt, error = %e, "punch signaling retrying");
                tokio::time::sleep(SIGNAL_RETRY_GAP).await;
            }
            Err(e) => {
                emit_punch_fail(swarm, peer, format!("signaling: {e}"));
                return Err(chained(io::Error::other(format!(
                    "punch signaling failed: {e}"
                ))));
            }
        }
    }
    Ok(ack.expect("retry loop must end with Ok on final attempt"))
}

/// 发起侧状态推进：RequestSent。生产路径上恒成功（Idle 发起即合法），
/// 失败分支只有单测可达，独立成 fn 供回归测试断言错误链。
pub(super) fn stage_request_sent(session: &mut PunchSession) -> io::Result<()> {
    session.mark_request_sent().map_err(chained)
}

/// 发起侧状态推进：Probing（收 Ack）。同上，失败分支只有单测可达。
pub(super) fn stage_on_ack(session: &mut PunchSession) -> io::Result<()> {
    session.on_ack().map_err(chained)
}

/// 探测对端 Ack 携带的宣告地址；直连落地即返回，全部失败返回 None。
async fn probe_for_direct(
    swarm: &Swarm,
    peer: PeerId,
    addrs: &[String],
) -> io::Result<Option<Mux>> {
    for entry in addrs {
        let Some(addr) = parse_probe_addr(entry) else {
            tracing::debug!(%peer, entry = %entry, "unparseable ack addr ignored");
            continue;
        };
        let attempt = tokio::time::timeout(PROBE_TIMEOUT, dial_one(swarm, peer, &addr));
        match attempt.await {
            Ok(Ok(mux)) => {
                tracing::info!(%peer, %addr, "punch probe landed direct connection");
                swarm.metrics.hop_ok(DialHop::Punch);
                swarm.emit(NodeEvent::DialHop {
                    peer,
                    hop: DialHop::Punch,
                    ok: true,
                    detail: format!("probe landed {addr}"),
                });
                return Ok(Some(mux));
            }
            Ok(Err(err)) => tracing::debug!(%peer, %addr, error = %err, "punch probe refused"),
            Err(_) => tracing::debug!(%peer, %addr, "punch probe timed out"),
        }
    }
    Ok(None)
}

/// 接入发起侧预留的电路；超时或链路失败原样上抛。
pub(super) async fn join_circuit(
    client: &mut RelayClient,
    circuit: u64,
) -> io::Result<BoxedStream> {
    tokio::time::timeout(JOIN_TIMEOUT, client.connect(CircuitId(circuit)))
        .await
        .map_err(|_| io::Error::other("circuit join timed out"))?
        .map_err(chained)
}

/// 电路流出站安全升级：Noise XX（expected 绑定对端）+ yamux（发起侧）。
pub(super) async fn secure_outbound(
    swarm: &Swarm,
    stream: BoxedStream,
    peer: PeerId,
) -> io::Result<Mux> {
    let (remote, enc) = NoiseXx::new()
        .outbound(stream, &swarm.keypair, Some(peer))
        .await
        .map_err(chained)?;
    if remote != peer {
        return Err(io::Error::other(format!(
            "circuit peer mismatch: expected {peer}, got {remote}"
        )));
    }
    Ok(Arc::new(YamuxMux::new(enc, true, MAX_STREAMS_PER_CONN)))
}

fn emit_punch_fail(swarm: &Swarm, peer: PeerId, detail: String) {
    swarm.metrics.hop_fail(DialHop::Punch);
    swarm.emit(NodeEvent::DialHop {
        peer,
        hop: DialHop::Punch,
        ok: false,
        detail,
    });
}
