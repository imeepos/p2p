//! 保活与空闲回收参数（E6 连接稳定性轮）+ relay 客户端保活任务。
//!
//! 客户端周期在控制流上发 KeepAlive、等服务端 KeepAliveAck；连续无应答达
//! 阈值即判定中继失联：置失联位令后续控制往返速断、关闭控制流写半（服务端
//! 走既有 EOF 回收路径），并以 ControlClosed 事件（复用既有事件变体，冻结
//! 契约只增不改）+ WARN 日志上抛，供上层重连或换中继。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::client::RelayEvent;
use crate::error::RelayError;
use crate::frame::write_msg;
use crate::health::RelayHealth;
use crate::messages::{relay_msg::Kind, RelayMsg};
use crate::state::CtrlWrite;

/// 保活间隔/超时/失联阈值、服务端静默上限与桥接空闲 TTL（全部可配）。
#[derive(Debug, Clone)]
pub struct RelayKeepalive {
    /// 客户端保活间隔。默认 10s：远小于常见 NAT UDP 映射寿命（约 30-60s），
    /// 保活兼防 NAT 表项过期；开销为每链路 6 帧/分钟。
    pub interval: Duration,
    /// 单次保活往返超时。默认 5s：E3 采样跨公网 relay RTT 为毫秒级，5s 含
    /// 数十倍抖动余量；仍无应答记一次失联计数。
    pub timeout: Duration,
    /// 连续失联判定阈值。默认 3：连续 3 次无应答（约 interval×3 = 30s）才
    /// 判失联，单次抖动不误杀。
    pub max_missed: u32,
    /// 服务端控制流静默上限：多久收不到任何帧即按客户端失联清理（与控制流
    /// 关闭同语义）。默认 45s：须大于客户端失联窗口（interval×max_missed
    /// = 30s），留出网络排队与调度抖动余量，避免健康客户端被误清。
    pub server_silence: Duration,
    /// 已桥接电路空闲回收 TTL：最近一次收发后持续静默满该时长即拆桥回收。
    /// 默认 120s：覆盖请求-响应式电路的自然间隙（E3 实测 ping rtt 秒级），
    /// 又不让死桥滞留占住全站槽位至 reserve TTL（最长 1h）。
    pub idle_circuit_ttl: Duration,
}

impl Default for RelayKeepalive {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(10),
            timeout: Duration::from_secs(5),
            max_missed: 3,
            server_silence: Duration::from_secs(45),
            idle_circuit_ttl: Duration::from_secs(120),
        }
    }
}

/// 控制流共享态：写半 + 待应答槽 + 失联/关闭两位（客户端侧）。
pub(crate) struct CtrlInner {
    pub(crate) write: Arc<CtrlWrite>,
    pub(crate) pending: PendingSlot,
    /// 保活判失联后置位：后续控制往返速断，不许静默重开控制流。
    pub(crate) lost: AtomicBool,
    /// 读半退出（EOF/错误）置位：保活任务据此安静退出，不重复上抛关闭事件。
    pub(crate) closed: AtomicBool,
    /// 健康记账：keepalive 记 RTT/load，上层选择器无锁读。
    pub(crate) health: Arc<RelayHealth>,
}

pub(crate) type PendingSlot = Arc<Mutex<Option<PendingReply>>>;

/// 待应答请求：期望回包形态 + 回传端。串包防御：形态不符不投递。
pub(crate) struct PendingReply {
    pub(crate) expect: ReplyExpect,
    pub(crate) tx: oneshot::Sender<RelayMsg>,
}

/// 期望回包形态（Reject 恒匹配任意请求）。
#[derive(Clone, Copy)]
pub(crate) enum ReplyExpect {
    Reserved,
    PunchAck,
    KeepAliveAck,
}

impl PendingReply {
    pub(crate) fn matches(&self, kind: &Kind) -> bool {
        let specific = match self.expect {
            ReplyExpect::Reserved => matches!(kind, Kind::Reserved(_)),
            ReplyExpect::PunchAck => matches!(kind, Kind::PunchAck(_)),
            ReplyExpect::KeepAliveAck => matches!(kind, Kind::KeepAliveAck(_)),
        };
        specific || matches!(kind, Kind::Reject(_))
    }
}

/// 控制往返串行锁：保活探测与用户请求互斥，互不吞包。
pub(crate) type RoundtripLock = Arc<Mutex<()>>;

/// 单次控制往返：置槽、写帧、限时等回。失败路径全部显式上抛。
pub(crate) async fn roundtrip(
    inner: &Arc<CtrlInner>,
    lock: &RoundtripLock,
    msg: RelayMsg,
    expect: ReplyExpect,
    what: &'static str,
    limit: Duration,
) -> Result<RelayMsg, RelayError> {
    let _guard = lock.lock().await;
    if inner.lost.load(Ordering::Relaxed) {
        tracing::warn!(op = what, "control op after relay lost; failing fast");
        return Err(RelayError::LinkClosed);
    }
    let (tx, rx) = oneshot::channel();
    *inner.pending.lock().await = Some(PendingReply { expect, tx });
    write_msg(&mut *inner.write.lock().await, &msg).await?;
    let reply = match tokio::time::timeout(limit, rx).await {
        Ok(reply) => reply,
        Err(_) => {
            // 超时清槽：迟到回包落进空槽只留告警，不会投给下一个请求
            *inner.pending.lock().await = None;
            tracing::warn!(waiting = what, "control roundtrip timed out");
            return Err(RelayError::Timeout(what));
        }
    };
    reply.map_err(|_| RelayError::LinkClosed)
}

/// 启动客户端保活任务；返回句柄供 RelayClient drop 时中止。
pub(crate) fn spawn_keepalive(
    inner: Arc<CtrlInner>,
    lock: RoundtripLock,
    events: mpsc::Sender<RelayEvent>,
    cfg: RelayKeepalive,
    peer: String,
) -> JoinHandle<()> {
    tokio::spawn(keepalive_loop(inner, lock, events, cfg, peer))
}

async fn keepalive_loop(
    inner: Arc<CtrlInner>,
    lock: RoundtripLock,
    events: mpsc::Sender<RelayEvent>,
    cfg: RelayKeepalive,
    peer: String,
) {
    let mut missed = 0u32;
    loop {
        if inner.closed.load(Ordering::Relaxed) {
            return; // 读半已退出：关闭事件由读半上抛，不重复发
        }
        tokio::time::sleep(cfg.interval).await;
        let started = Instant::now();
        let probe = roundtrip(
            &inner,
            &lock,
            RelayMsg::keep_alive(),
            ReplyExpect::KeepAliveAck,
            "keepalive",
            cfg.timeout,
        )
        .await;
        match probe {
            Ok(reply) => {
                inner.health.note_rtt(started.elapsed());
                if let Some(Kind::KeepAliveAck(a)) = reply.kind {
                    inner.health.note_load(a.load_permille);
                }
                if missed != 0 {
                    tracing::debug!(peer = %peer, recovered_after = missed, "relay keepalive recovered");
                }
                missed = 0;
            }
            Err(e) => {
                missed += 1;
                tracing::warn!(
                    peer = %peer, missed, max_missed = cfg.max_missed, error = %e,
                    "relay keepalive probe missed"
                );
                if missed >= cfg.max_missed {
                    declare_lost(&inner, &events, &peer, missed, &cfg).await;
                    return;
                }
            }
        }
    }
}

/// 判失联三连：WARN 日志、关控制流写半（服务端走既有 EOF 回收）、事件上抛。
async fn declare_lost(
    inner: &Arc<CtrlInner>,
    events: &mpsc::Sender<RelayEvent>,
    peer: &str,
    missed: u32,
    cfg: &RelayKeepalive,
) {
    let reason = format!("relay lost: keepalive missed x{missed}");
    tracing::warn!(
        peer = %peer,
        missed,
        interval_ms = cfg.interval.as_millis() as u64,
        timeout_ms = cfg.timeout.as_millis() as u64,
        "{reason}; release registrations/circuits bound to this relay, reconnect or switch relay"
    );
    inner.lost.store(true, Ordering::Relaxed);
    let _ = inner.write.lock().await.shutdown().await;
    let _ = events.send(RelayEvent::ControlClosed { reason }).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_server_silence_above_client_loss_window() {
        let k = RelayKeepalive::default();
        let loss_window = k.interval * k.max_missed;
        assert!(
            k.server_silence > loss_window,
            "服务端静默上限必须大于客户端失联窗口，否则健康客户端先被服务端误清"
        );
        assert!(!k.idle_circuit_ttl.is_zero());
    }
}
