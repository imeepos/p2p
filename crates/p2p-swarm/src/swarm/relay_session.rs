//! relay 降级链会话（design §7.3，M3）：每条 relay 地址一个常驻会话。
//!
//! 职责：维持到 relay 的控制链路（断线按退避重连）；转发入站 PunchReq 给
//! 应答侧；对外提供 Degrade 命令（reserve + 打洞信令 + 探测 + 电路兜底）。
//! 每一跳结果发 DialHop 事件/日志，禁止静默降级（design §12）。
//!
//! 地址约定：信令 addrs 沿用 TransportAddr 展示格式（ip/u端口、ip/t端口），
//! 电路接入项为 cid/<N>——PunchReq/Ack 帧形状冻结，电路号借地址串传递，
//! 两端均为本栈节点时自洽。

use std::io;
use std::sync::Arc;
use std::time::Duration;

use p2p_identity::PeerId;
use p2p_mux::{BoxedStream, MuxControl, YamuxMux, MAX_STREAMS_PER_CONN};
use p2p_relay::{PunchSession, RelayClient, RelayEvent, RelayLink};
use p2p_security::{NoiseXx, SecurityUpgrade};
use p2p_transport::{Transport, TransportAddr};
use tokio::sync::mpsc;

use super::dial::dial_one;
use super::responder::{handle_punch_req, parse_probe_addr};
use super::{Mux, Swarm};
use crate::{Backoff, DialHop, NodeEvent};

/// 电路预留 TTL；对端未在期内接入即由 relay 回收。
const CIRCUIT_TTL: Duration = Duration::from_secs(120);
/// 控制通道注册载体电路的 TTL（取上限，控制流关闭即失效）。
const CONTROL_REG_TTL: Duration = Duration::from_secs(3600);
/// 等待对端接入电路上限。
const JOIN_TIMEOUT: Duration = Duration::from_secs(10);
/// 探测单地址上限（应答侧共用）。
pub(super) const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// 打洞信令有界重试：对端会话可能仍在重连窗口内。
const SIGNAL_RETRIES: u32 = 5;
/// 信令重试间隔。
const SIGNAL_RETRY_GAP: Duration = Duration::from_millis(400);
/// 命令队列容量。
const CMD_CAPACITY: usize = 16;
/// 电路接入项前缀。
pub(super) const CID_PREFIX: &str = "cid/";

/// 拨号器交给会话的降级请求：打洞失败即落到电路兜底。
pub(super) enum RelayCmd {
    Degrade {
        peer: PeerId,
        reply: tokio::sync::oneshot::Sender<io::Result<Mux>>,
    },
}

/// 真实传输上的 relay 链路：一条已认证连接 = 一条 RelayLink，
/// peer_id 取握手互认的身份（属主/配额记账依据，红线见 coordination.md）。
struct TransportRelayLink {
    peer: String,
    mux: Arc<dyn MuxControl>,
}

#[async_trait::async_trait]
impl RelayLink for TransportRelayLink {
    fn peer_id(&self) -> &str {
        &self.peer
    }

    async fn open_stream(&self) -> io::Result<BoxedStream> {
        self.mux.open_stream().await
    }

    async fn accept_stream(&self) -> Option<BoxedStream> {
        self.mux.accept_stream().await
    }
}

/// 为每条 relay 地址派生常驻会话任务。
pub(super) fn spawn_sessions(swarm: &Arc<Swarm>, addrs: Vec<TransportAddr>) {
    if addrs.is_empty() {
        tracing::debug!("no relay configured; degradation chain stops at direct");
        return;
    }
    let count = addrs.len();
    for addr in addrs {
        let (tx, rx) = mpsc::channel::<RelayCmd>(CMD_CAPACITY);
        swarm.add_relay_session(tx);
        tokio::spawn(session_loop(swarm.clone(), addr, rx));
    }
    tracing::info!(count, "relay fallback wired");
}

async fn session_loop(swarm: Arc<Swarm>, addr: TransportAddr, mut cmds: mpsc::Receiver<RelayCmd>) {
    let mut shutdown = swarm.shutdown_rx.clone();
    let mut backoff = Backoff::default();
    loop {
        if swarm.is_stopping() {
            return;
        }
        let link = match dial_relay_link(&swarm, &addr).await {
            Ok(link) => {
                backoff.reset();
                link
            }
            Err(err) => {
                tracing::warn!(%addr, error = %err, "relay link dial failed; backing off");
                swarm.metrics.count_reconnect();
                tokio::time::sleep(backoff.next_delay()).await;
                continue;
            }
        };
        tracing::info!(%addr, "relay session connected");
        let mut client = RelayClient::new(link);
        // relay 以首帧 Reserve 登记控制通道：自用电路（仅本端可接入）作信令注册载体，
        // 否则纯被叫方永远收不到 PunchReq 转发。控制流存活期内注册持续有效。
        if let Err(e) = client.reserve(CONTROL_REG_TTL, "").await {
            tracing::warn!(%addr, error = %e, "control registration failed; reconnecting");
            swarm.metrics.count_reconnect();
            tokio::time::sleep(backoff.next_delay()).await;
            continue;
        }
        let close_reason = loop {
            tokio::select! {
                _ = shutdown.changed() => return,
                cmd = cmds.recv() => match cmd {
                    Some(RelayCmd::Degrade { peer, reply }) => {
                        let out = degrade(&swarm, &mut client, peer).await;
                        let _ = reply.send(out);
                    }
                    None => return,
                },
                ev = client.next_event() => match ev {
                    Some(RelayEvent::PunchReq(req)) => handle_punch_req(&swarm, &mut client, req).await,
                    Some(RelayEvent::PunchAck(_)) => {
                        tracing::debug!(%addr, "unsolicited punch ack ignored");
                    }
                    Some(RelayEvent::ControlClosed { reason }) => break reason,
                    None => return,
                },
            }
        };
        tracing::warn!(%addr, reason = %close_reason, "relay control closed; reconnecting");
        swarm.metrics.count_reconnect();
        tokio::time::sleep(backoff.next_delay()).await;
    }
}

/// 拨 relay（盲拨例外：relay 仅以地址配置、身份未知，显式留痕后信任加密信道）。
async fn dial_relay_link(swarm: &Swarm, addr: &TransportAddr) -> io::Result<Box<dyn RelayLink>> {
    let transport: &dyn Transport = match addr {
        TransportAddr::Quic { .. } => &swarm.dial_quic,
        TransportAddr::Tcp { .. } => &swarm.dial_tcp,
    };
    tracing::warn!(%addr, "relay blind dial: relay peer unknown, no expected binding");
    let conn = transport
        .dial(addr, &swarm.keypair, None)
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(Box::new(TransportRelayLink {
        peer: conn.remote.to_string(),
        mux: conn.mux,
    }))
}

/// 降级链 2/3 跳：预留电路并随信令通告，探测对端地址，失败接入选定电路。
async fn degrade(swarm: &Swarm, client: &mut RelayClient, peer: PeerId) -> io::Result<Mux> {
    let cid = client
        .reserve(CIRCUIT_TTL, &peer.to_string())
        .await
        .map_err(|e| {
            emit_punch_fail(swarm, peer, format!("relay reserve: {e}"));
            io::Error::other(e.to_string())
        })?;
    let mut offer = swarm.punch_addrs_strs();
    offer.push(format!("{CID_PREFIX}{}", cid.0));

    let mut session = PunchSession::initiator(peer.to_string());
    // 信令有界重试：对端控制链路可能尚未在 relay 上就绪
    let ack = {
        let mut ack = None;
        for attempt in 1..=SIGNAL_RETRIES {
            match client.request_punch(&peer.to_string(), offer.clone()).await {
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
                    return Err(io::Error::other(format!("punch signaling failed: {e}")));
                }
            }
        }
        ack.expect("retry loop must end with Ok on final attempt")
    };
    session
        .mark_request_sent()
        .map_err(|e| io::Error::other(e.to_string()))?;
    // Ack 已到手：双方进入探测时点，探测对端 Ack 携带的宣告地址
    session
        .on_ack()
        .map_err(|e| io::Error::other(e.to_string()))?;

    for entry in &ack.addrs {
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
                return Ok(mux);
            }
            Ok(Err(err)) => tracing::debug!(%peer, %addr, error = %err, "punch probe refused"),
            Err(_) => tracing::debug!(%peer, %addr, "punch probe timed out"),
        }
    }
    emit_punch_fail(
        swarm,
        peer,
        "probes failed; falling back to relay circuit".to_string(),
    );

    // 中继兜底：接入电路（对端按信令接入即配对），密文流上完成安全握手
    let stream = tokio::time::timeout(JOIN_TIMEOUT, client.connect(cid))
        .await
        .map_err(|_| io::Error::other("circuit join timed out"))?
        .map_err(|e| io::Error::other(e.to_string()))?;
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

fn emit_punch_fail(swarm: &Swarm, peer: PeerId, detail: String) {
    swarm.metrics.hop_fail(DialHop::Punch);
    swarm.emit(NodeEvent::DialHop {
        peer,
        hop: DialHop::Punch,
        ok: false,
        detail,
    });
}

/// 电路流出站安全升级：Noise XX（expected 绑定对端）+ yamux（发起侧）。
async fn secure_outbound(swarm: &Swarm, stream: BoxedStream, peer: PeerId) -> io::Result<Mux> {
    let (remote, enc) = NoiseXx::new()
        .outbound(stream, &swarm.keypair, Some(peer))
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;
    if remote != peer {
        return Err(io::Error::other(format!(
            "circuit peer mismatch: expected {peer}, got {remote}"
        )));
    }
    Ok(Arc::new(YamuxMux::new(enc, true, MAX_STREAMS_PER_CONN)))
}
