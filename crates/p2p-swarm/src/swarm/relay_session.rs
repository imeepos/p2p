//! relay 降级链会话（design §7.3，M3）：每条 relay 地址一个常驻会话。
//!
//! 职责：维持到 relay 的控制链路（断线按退避重连）；转发入站 PunchReq 给
//! 应答侧；对外提供 Degrade 命令入口（单次降级实现在 degrade_hop）。
//! 控制链路就绪后共享健康句柄，负载感知派发见 relay_degrade/relay_selector。
//!
//! 地址约定：信令 addrs 沿用 TransportAddr 展示格式（ip/u端口、ip/t端口），
//! 电路接入项为 cid/<N>——PunchReq/Ack 帧形状冻结，电路号借地址串传递，
//! 两端均为本栈节点时自洽。

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use p2p_identity::PeerId;
use p2p_mux::{BoxedStream, MuxControl};
use p2p_relay::{RelayClient, RelayEvent, RelayHealth, RelayLink};
use p2p_transport::{Transport, TransportAddr};
use tokio::sync::mpsc;

use super::degrade_hop::degrade;
use super::relay_degrade::RelaySessionHandle;
use super::responder::handle_punch_req;
use super::{Mux, Swarm};
use crate::Backoff;

/// 控制通道注册载体电路的 TTL（取上限，控制流关闭即失效）。
const CONTROL_REG_TTL: Duration = Duration::from_secs(3600);
/// 命令队列容量。
const CMD_CAPACITY: usize = 16;
/// 会话最短健康存活时长：满值才复位退避（E5 语义，闪断不复位）。
const MIN_HEALTHY_SESSION: Duration = Duration::from_secs(10);

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
        let health = Arc::new(Mutex::new(None));
        swarm.add_relay_session(RelaySessionHandle {
            tx,
            addr: addr.clone(),
            health: health.clone(),
        });
        tokio::spawn(session_loop(swarm.clone(), addr, rx, health));
    }
    tracing::info!(count, "relay fallback wired");
}

async fn session_loop(
    swarm: Arc<Swarm>,
    addr: TransportAddr,
    mut cmds: mpsc::Receiver<RelayCmd>,
    health_slot: Arc<Mutex<Option<Arc<RelayHealth>>>>,
) {
    let mut shutdown = swarm.shutdown_rx.clone();
    let mut backoff = Backoff::default();
    // 本轮链路建立时刻；退避仅在会话健康存活满时长后复位（E5，防闪断钉死 base 值）
    let mut up_since: Option<std::time::Instant> = None;
    loop {
        if swarm.is_stopping() {
            return;
        }
        let link = match dial_relay_link(&swarm, &addr).await {
            Ok(link) => {
                // 首次建链直接复位；重连须会话健康存活满时长才复位（E5 语义）
                let healthy = match up_since {
                    Some(t) => t.elapsed() >= MIN_HEALTHY_SESSION,
                    None => true,
                };
                if healthy {
                    backoff.reset();
                    tracing::debug!(%addr, "backoff reset after healthy session");
                }
                up_since = Some(std::time::Instant::now());
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
        // 控制链路就绪：共享健康句柄（keepalive 周期刷新 RTT/load，选择器读取）
        *health_slot.lock().expect("health slot") = client.health_handle();
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
