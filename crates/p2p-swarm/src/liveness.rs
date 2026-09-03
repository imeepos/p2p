//! 统一对端活跃度判定（E8 调研建议 5）：mDNS TTL 过期、rendezvous TTL 过期、
//! relay 槽失活、探活判死四源信号收拢为单一判定入口，对外只发一种活跃度事件。
//!
//! 与连接状态机的关系（任务要求 3，务必先读）：
//! - 状态机（PeerMachine/PeerDown/BackingOff）只认传输事实：入池即 Connected、
//!   确认出池才断链。本模块是观测面衍生，不驱动任何状态转移，也不被状态机驱动；
//!   两者并存、不合并。
//! - PeerDown 仍是探活判死在状态机侧的事件（语义不变）；PeerLiveness 是同一
//!   判死在观测面的镜像，且额外合并发现面/中继面信号。上层要「连接事实」订
//!   状态机事件，要「一致的对端在线/离线观测」订 PeerLiveness。
//! - 直连（非中继）断链不进本判定：它是连接事实，由状态机经 LinkLost 全权
//!   处理；再进活跃度只会把同一次断链播报两遍。中继电路断链对上层不可见
//!   （无直连降级语义），故以 RelaySlot 源进入本判定。
//!
//! 来源粒度说明：mDNS 与 rendezvous 的 TTL 过期在既有冻结接缝处已合并——
//! facade 只读，两者统一经 DiscoveryEvent::Expired 转发到 Swarm::on_peer_expired，
//! 接缝上不再可分，故共用 Discovery 源；来源归属细节可由地址簿来源佐证，
//! 不影响「单一判定」目标。
//!
//! 判定语义：每个对端维护确定的活跃态（Unknown/Alive/Dead），只在翻转时发
//! 事件——Dead 判定与 Dead 恢复 Alive 各至多一条；同一对端多源信号并发时，
//! 首个改变状态的信号产出唯一判定，其余只更新记账（不得重复翻转）。
//! Unknown 首次见到活信号只记账不发事件：与 PeerDiscovered/PeerConnected
//! 语义重叠，不重复播报。

use std::collections::HashMap;
use std::sync::Mutex;

use p2p_identity::PeerId;
use tokio::sync::broadcast;

use crate::lifecycle::LifecycleEvent;

/// 活跃度信号源。四类判死输入中，发现面（mDNS/rendezvous TTL 过期）共用
/// Discovery（接缝已合并，见模块注释）；Connection 是最强的活信号
/// （连接建成）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LivenessSource {
    /// 发现面 TTL 过期/刷新（mDNS、rendezvous 共用入口）。
    Discovery,
    /// 中继电路（relay 槽）失活或建成。
    RelaySlot,
    /// 探活往返命中或判死。
    Probe,
    /// 连接面正信号：入池建成或业务流到达（收包刷新 last_recv 语义）。
    Connection,
}

impl LivenessSource {
    pub fn as_str(self) -> &'static str {
        match self {
            LivenessSource::Discovery => "discovery",
            LivenessSource::RelaySlot => "relay_slot",
            LivenessSource::Probe => "probe",
            LivenessSource::Connection => "connection",
        }
    }
}

/// 统一活跃度判定事件（唯一事件类型，秒级时间戳）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerLiveness {
    pub peer: PeerId,
    /// true=恢复在线，false=判定失联。
    pub alive: bool,
    /// 触发本次判定的信号源。
    pub source: LivenessSource,
    /// 最后正信号（收包/探活命中/发现刷新）的 UNIX 秒；无正信号时为判定时刻。
    pub last_seen_unix: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Judged {
    Unknown,
    Alive,
    Dead,
}

#[derive(Clone, Copy)]
struct PeerRecord {
    judged: Judged,
    last_seen_unix: u64,
}

/// 活跃度判定账本：单一写入口，状态翻转时发 PeerLiveness 到生命周期通道。
pub struct LivenessBook {
    entries: Mutex<HashMap<PeerId, PeerRecord>>,
    events: broadcast::Sender<LifecycleEvent>,
}

impl LivenessBook {
    pub(crate) fn new(events: broadcast::Sender<LifecycleEvent>) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            events,
        }
    }

    /// 锁中毒按原样恢复（对齐 cache.rs L3 口径：网络路径零 panic）。
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<PeerId, PeerRecord>> {
        self.entries.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// 活信号：刷新最后活跃时刻；Dead 态收到即恢复并发翻转事件。
    pub fn note_alive(&self, peer: PeerId, source: LivenessSource, now_unix: u64) {
        let judged = {
            let mut map = self.lock();
            let rec = map.entry(peer).or_insert(PeerRecord {
                judged: Judged::Unknown,
                last_seen_unix: now_unix,
            });
            rec.last_seen_unix = now_unix;
            let recovered = rec.judged == Judged::Dead;
            rec.judged = Judged::Alive;
            recovered.then_some(true)
        };
        // Unknown→Alive 静默（与发现/连接事件重叠）；Dead→Alive 是恢复翻转
        if let Some(alive) = judged {
            self.emit(PeerLiveness {
                peer,
                alive,
                source,
                last_seen_unix: now_unix,
            });
        }
    }

    /// 死信号：仅当当前非 Dead 才产出判定（多源并发只产出一条），
    /// 其余信号只留记账。last_seen 取最近正信号，无正信号用判定时刻。
    pub fn note_dead(&self, peer: PeerId, source: LivenessSource, now_unix: u64) {
        let emit = {
            let mut map = self.lock();
            let rec = map.entry(peer).or_insert(PeerRecord {
                judged: Judged::Unknown,
                last_seen_unix: now_unix,
            });
            let last_seen = rec.last_seen_unix;
            if rec.judged != Judged::Dead {
                rec.judged = Judged::Dead;
                Some(last_seen)
            } else {
                None
            }
        };
        if let Some(last_seen) = emit {
            self.emit(PeerLiveness {
                peer,
                alive: false,
                source,
                last_seen_unix: last_seen,
            });
        }
    }

    fn emit(&self, ev: PeerLiveness) {
        // 无订阅者丢弃属正常态（对齐 NodeEvent emit 口径）；
        // 判定记账已在锁内落定，通道失败不回滚判定。
        let _ = self.events.send(LifecycleEvent::PeerLiveness(ev));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> (LivenessBook, broadcast::Receiver<LifecycleEvent>) {
        let (tx, rx) = broadcast::channel(64);
        (LivenessBook::new(tx), rx)
    }

    fn peer(b: u8) -> PeerId {
        PeerId::from_bytes([b; 32])
    }

    fn liveness_events(rx: &mut broadcast::Receiver<LifecycleEvent>) -> Vec<PeerLiveness> {
        let mut out = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            if let LifecycleEvent::PeerLiveness(p) = ev {
                out.push(p);
            }
        }
        out
    }

    #[test]
    fn concurrent_dead_signals_produce_single_judgment() {
        let (book, mut rx) = book();
        book.note_alive(peer(1), LivenessSource::Discovery, 100);
        // 三源并发判死：只有首个产出事件
        book.note_dead(peer(1), LivenessSource::Discovery, 110);
        book.note_dead(peer(1), LivenessSource::RelaySlot, 110);
        book.note_dead(peer(1), LivenessSource::Probe, 111);
        let evs = liveness_events(&mut rx);
        assert_eq!(evs.len(), 1, "multi-source dead must judge once: {evs:?}");
        assert!(!evs[0].alive);
        assert_eq!(evs[0].source, LivenessSource::Discovery);
        assert_eq!(evs[0].last_seen_unix, 100);
    }

    #[test]
    fn dead_then_alive_flips_once_each_way() {
        let (book, mut rx) = book();
        book.note_dead(peer(2), LivenessSource::Probe, 50);
        book.note_dead(peer(2), LivenessSource::Discovery, 51);
        book.note_alive(peer(2), LivenessSource::Connection, 60);
        book.note_alive(peer(2), LivenessSource::Probe, 61);
        let evs = liveness_events(&mut rx);
        assert_eq!(evs.len(), 2, "dead + recovery only: {evs:?}");
        assert!(!evs[0].alive && evs[0].source == LivenessSource::Probe);
        assert!(evs[1].alive && evs[1].source == LivenessSource::Connection);
    }

    #[test]
    fn first_alive_signal_is_silent_bookkeeping_only() {
        let (book, mut rx) = book();
        book.note_alive(peer(3), LivenessSource::Discovery, 10);
        assert!(
            liveness_events(&mut rx).is_empty(),
            "Unknown->Alive must not emit"
        );
        // 但记账在：随后的死判定携带该 last_seen
        book.note_dead(peer(3), LivenessSource::Discovery, 15);
        let evs = liveness_events(&mut rx);
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].last_seen_unix, 10);
    }
}
