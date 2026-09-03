//! 对端生命周期监督·状态与消息（E6）：每 Peer 一份状态机 + 退避 + 探测记账。
//!
//! 划分：本文件持有共享状态与排程工具；消息裁决见 lifecycle_handlers；
//! 事件循环与任务派发见 lifecycle_task；探测往返见 ping。
//! 所有失败路径（探测未命中、重连失败、非法转移）都留日志或事件（design §12）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use p2p_identity::PeerId;
use tokio::sync::{broadcast, mpsc};

use crate::lifecycle::{ConnState, LifecycleEvent, PeerLifecycleConfig, PeerMachine};
use crate::liveness::LivenessSource;
use crate::usage::unix_now;
use crate::Backoff;

use super::Swarm;

/// Swarm 侧生命周期查询入口（E8 自 mod.rs 迁入，语义归位）。
impl Swarm {
    /// E6：peer 生命周期状态（未跟踪返回 None）。
    pub fn peer_state(&self, peer: &PeerId) -> Option<ConnState> {
        self.lifecycle.state_of(peer)
    }

    /// E6：已排定的下次重连退避时长（BackingOff 态有值；观测与测试断言用）。
    pub fn peer_scheduled_backoff(&self, peer: &PeerId) -> Option<Duration> {
        self.lifecycle.scheduled_backoff(peer)
    }

    /// E6：订阅对端生命周期事件（状态转移/PeerDown/PeerUp，E8 增 ConnectionClosed
    /// 与 PeerLiveness 两种加法事件）。
    pub fn subscribe_lifecycle(&self) -> broadcast::Receiver<LifecycleEvent> {
        self.lifecycle.events.subscribe()
    }

    /// E8：relay 槽失活上报（统一活跃度判定的死信号入口之一）。serve 循环
    /// 在中继电路出池时走同一账本；本方法是等价的公共上报口，供无法持有
    /// 账本句柄的调用方（测试、运维探针）使用。
    pub fn on_relay_slot_lost(&self, peer: PeerId) {
        self.liveness
            .note_dead(peer, LivenessSource::RelaySlot, unix_now());
    }
}

/// 消息通道容量；监督者常驻排空，通道拒绝仅留告警（关停期正常）。
pub(super) const CHANNEL_CAPACITY: usize = 128;

/// 监督者消息：来源只有 swarm 钩子与监督者自派任务。
pub(super) enum LifecycleMsg {
    /// 连接入池（出站拨号或入站 accept 的胜者，双向唯一入口）。
    Connected { peer: PeerId },
    /// 在册连接退出（serve 循环 remove_if_same 命中）。
    LinkLost { peer: PeerId },
    /// 用户显式 connect 开始（未跟踪 peer 首拨时建档）。
    DialStart { peer: PeerId },
    /// 用户显式 connect 失败（仅对从未连上的 peer 生效）。
    DialFailed { peer: PeerId },
    /// 用户挂断：停止跟踪与重连。
    HungUp { peer: PeerId },
    /// 探测结果（run_probe 回报）。
    Probed {
        peer: PeerId,
        ok: bool,
        detail: String,
    },
    /// 重连拨号结果（run_reconnect 回报）。
    Reconnected { peer: PeerId, ok: bool },
}

/// 每 Peer 的生命周期记账。
pub(super) struct Entry {
    pub(super) machine: PeerMachine,
    pub(super) backoff: Backoff,
    /// 连续探测未命中次数。
    pub(super) misses: u32,
    /// 本段会话起始时刻（Connected 起；断开时据此记录存活时长）。
    pub(super) up_since: Option<std::time::Instant>,
    /// 上一段会话存活时长；None = 尚无会话史（首连成功前不发 PeerUp）。
    pub(super) last_uptime: Option<Duration>,
    /// 探测在途（防重复派发）。
    pub(super) probing: bool,
    /// 重连拨号在途。
    pub(super) dialing: bool,
    /// 下次探测时点（tokio 时钟）。
    pub(super) next_probe_at: Option<tokio::time::Instant>,
    /// 下次重连时点（tokio 时钟）。
    pub(super) reconnect_at: Option<tokio::time::Instant>,
    /// 已排定的退避时长（观测：peer_scheduled_backoff）。
    pub(super) scheduled: Option<Duration>,
}

impl Entry {
    pub(super) fn new(cfg: &PeerLifecycleConfig) -> Self {
        Self {
            machine: PeerMachine::new(),
            backoff: Backoff::new(cfg.reconnect_base, cfg.reconnect_max),
            misses: 0,
            up_since: None,
            last_uptime: None,
            probing: false,
            dialing: false,
            next_probe_at: None,
            reconnect_at: None,
            scheduled: None,
        }
    }
}

/// 监督者共享状态：监督任务是唯一写者，Swarm 查询方法只读。
pub(super) struct LifecycleShared {
    pub(super) entries: HashMap<PeerId, Entry>,
    pub(super) cfg: PeerLifecycleConfig,
}

/// Swarm 持有的生命周期句柄：notify 非阻塞，查询走共享快照。
#[derive(Clone)]
pub(super) struct LifecycleHandle {
    pub(super) tx: mpsc::Sender<LifecycleMsg>,
    /// E6 独立生命周期事件通道（NodeEvent 冻结流零扰动，等价事件机制）。
    pub(super) events: broadcast::Sender<LifecycleEvent>,
    pub(super) enabled: bool,
    pub(super) shared: Arc<Mutex<LifecycleShared>>,
}

impl LifecycleHandle {
    pub(super) fn new(
        cfg: PeerLifecycleConfig,
        events: broadcast::Sender<LifecycleEvent>,
    ) -> (Self, mpsc::Receiver<LifecycleMsg>) {
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
        let enabled = cfg.enabled;
        (
            Self {
                tx,
                events,
                enabled,
                shared: Arc::new(Mutex::new(LifecycleShared {
                    entries: HashMap::new(),
                    cfg,
                })),
            },
            rx,
        )
    }

    /// 钩子入口：非阻塞；关停期通道拒绝只留告警，不算业务失败。
    pub(super) fn notify(&self, msg: LifecycleMsg) {
        if !self.enabled {
            return;
        }
        if let Err(err) = self.tx.try_send(msg) {
            tracing::warn!(error = %err, "lifecycle channel rejected message");
        }
    }

    /// 读配置快照；禁止在已持有 shared 锁的路径调用（非重入锁）。
    pub(super) fn cfg(&self) -> PeerLifecycleConfig {
        self.shared.lock().expect("lifecycle lock").cfg.clone()
    }

    pub(super) fn state_of(&self, peer: &PeerId) -> Option<ConnState> {
        let shared = self.shared.lock().expect("lifecycle lock");
        shared.entries.get(peer).map(|e| e.machine.state())
    }

    pub(super) fn scheduled_backoff(&self, peer: &PeerId) -> Option<Duration> {
        let shared = self.shared.lock().expect("lifecycle lock");
        shared.entries.get(peer).and_then(|e| e.scheduled)
    }
}

pub(super) fn emit_state(
    events: &broadcast::Sender<LifecycleEvent>,
    peer: PeerId,
    from: ConnState,
    to: ConnState,
) {
    // 无订阅者时丢弃属正常态（对齐 mod.rs emit 口径）
    let _ = events.send(LifecycleEvent::PeerStateChanged { peer, from, to });
}

/// 排定下次重连：指数退避取值 + 抖动；warn 留痕（对端不可达属失败路径）。
pub(super) fn schedule_retry(
    cfg: &PeerLifecycleConfig,
    peer: PeerId,
    entry: &mut Entry,
    cause: &str,
) {
    let raw = entry.backoff.next_delay();
    let delay = jitter(raw, cfg.reconnect_jitter);
    entry.reconnect_at = Some(tokio::time::Instant::now() + delay);
    entry.scheduled = Some(delay);
    tracing::warn!(
        %peer,
        cause,
        delay = ?delay,
        attempts = entry.backoff.attempts(),
        "peer unreachable; reconnect scheduled"
    );
}

/// ±ratio 抖动；熵源取墙钟亚秒纳秒（仅防惊群，非加密用途）。
fn jitter(delay: Duration, ratio: f64) -> Duration {
    if ratio <= 0.0 {
        return delay;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let unit = f64::from(nanos % 1000) / 1000.0;
    delay.mul_f64(1.0 - ratio + 2.0 * ratio * unit)
}
