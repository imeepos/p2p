//! 对端生命周期监督·消息裁决（E6）：连接/断链/探测/重连/挂断的状态转移裁决。
//!
//! 纪律：持有 shared 锁期间不得再调 handle.cfg()（非重入锁），
//! 配置一律在加锁前取快照。非法转移拒绝并留 warn 日志。

use std::time::Instant;

use p2p_identity::PeerId;

use super::lifecycle::{emit_state, schedule_retry, Entry, LifecycleHandle, LifecycleMsg};
use crate::lifecycle::{ConnState, LifecycleEvent};
use crate::liveness::LivenessSource;
use crate::usage::unix_now;
use crate::CloseReason;

pub(super) fn handle_msg(
    handle: &LifecycleHandle,
    swarm: &std::sync::Weak<super::Swarm>,
    msg: LifecycleMsg,
) {
    match msg {
        LifecycleMsg::Connected { peer } => mark_connected(handle, peer),
        LifecycleMsg::LinkLost { peer } => on_link_lost(handle, peer),
        LifecycleMsg::DialStart { peer } => on_dial_start(handle, peer),
        LifecycleMsg::DialFailed { peer } => on_dial_failed(handle, peer),
        LifecycleMsg::HungUp { peer } => on_hung_up(handle, peer),
        LifecycleMsg::Probed { peer, ok, detail } => on_probed(swarm, handle, peer, ok, detail),
        LifecycleMsg::Reconnected { peer, ok } => on_reconnected(handle, peer, ok),
    }
}

/// 连接建成（双向唯一入口）。含复位触发点：见函数内注释。
pub(super) fn mark_connected(handle: &LifecycleHandle, peer: PeerId) {
    let mut shared = handle.shared.lock().expect("lifecycle lock");
    let cfg = shared.cfg.clone();
    let entry = shared
        .entries
        .entry(peer)
        .or_insert_with(|| Entry::new(&cfg));
    // 复位触发点（E5 候选「重连退避复位语义」正式落地）：
    // 连接建成时，若上一段会话存活满 reset_min_uptime（健康），退避序列归零——
    // 故障期确定结束，此后离线从 base 重新爬升。闪断（< min）不复位：
    // 防止「连上即断」的对端把重连间隔钉死在 base，形成紧密重连风暴。
    // 消融锚点：注释下方 if healthy 块，itest backoff_resets_after_healthy_reconnect 必红。
    let healthy = entry
        .last_uptime
        .is_some_and(|up| up >= cfg.reset_min_uptime);
    let from = match entry.machine.transition(ConnState::Connected) {
        Ok(from) => from,
        Err(err) => {
            // 新旧连接收敛竞速（旧连接未断时新连接入池）：刷新会话计时即可
            tracing::debug!(%peer, from = err.from.as_str(), "connected while already connected; session refreshed");
            entry.up_since = Some(Instant::now());
            entry.misses = 0;
            return;
        }
    };
    if healthy {
        entry.backoff.reset();
        tracing::info!(%peer, uptime = ?entry.last_uptime, "backoff reset: previous session healthy");
    }
    entry.misses = 0;
    entry.probing = false;
    entry.dialing = false;
    entry.reconnect_at = None;
    entry.scheduled = None;
    entry.up_since = Some(Instant::now());
    entry.next_probe_at = Some(tokio::time::Instant::now() + cfg.probe_interval);
    emit_state(&handle.events, peer, from, ConnState::Connected);
    // 恢复信号：有过下线史（last_uptime 已记录）的对端再次建成连接，与 PeerDown/PeerDisconnected 成对
    if entry.last_uptime.is_some() {
        let _ = handle.events.send(LifecycleEvent::PeerUp { peer });
    }
}

/// 在册连接退出（serve 循环确认本连接确已出池）。传输层断开无需探测：
/// 直接进入退避重连；断开事件由 serve 循环已发的 PeerDisconnected 承担。
pub(super) fn on_link_lost(handle: &LifecycleHandle, peer: PeerId) {
    let mut shared = handle.shared.lock().expect("lifecycle lock");
    let cfg = shared.cfg.clone();
    let Some(entry) = shared.entries.get_mut(&peer) else {
        tracing::debug!(%peer, "link lost for untracked peer; ignored");
        return;
    };
    if entry.machine.state() != ConnState::Connected {
        tracing::debug!(%peer, state = entry.machine.state().as_str(), "stale link lost; ignored");
        return;
    }
    entry.last_uptime = entry.up_since.map(|t| t.elapsed());
    match entry.machine.transition(ConnState::BackingOff) {
        Ok(from) => {
            emit_state(&handle.events, peer, from, ConnState::BackingOff);
            schedule_retry(&cfg, peer, entry, "link lost");
        }
        Err(err) => tracing::warn!(%peer, error = %err, "transition rejected on link lost"),
    }
}

/// 首拨建档：未跟踪 peer 的用户拨号记 Disconnected→Connecting（状态机全程可见）。
pub(super) fn on_dial_start(handle: &LifecycleHandle, peer: PeerId) {
    let mut shared = handle.shared.lock().expect("lifecycle lock");
    if shared.entries.contains_key(&peer) {
        tracing::debug!(%peer, "dial start on tracked peer; lifecycle state unchanged");
        return;
    }
    let cfg = shared.cfg.clone();
    let mut entry = Entry::new(&cfg);
    match entry.machine.transition(ConnState::Connecting) {
        Ok(from) => {
            shared.entries.insert(peer, entry);
            tracing::info!(%peer, "dial start: tracking peer lifecycle");
            emit_state(&handle.events, peer, from, ConnState::Connecting);
        }
        Err(err) => tracing::warn!(%peer, error = %err, "transition rejected on dial start"),
    }
}

/// 用户拨号失败：从未连上的 peer 不自动重连（无会话史，出册停止跟踪）。
pub(super) fn on_dial_failed(handle: &LifecycleHandle, peer: PeerId) {
    let mut shared = handle.shared.lock().expect("lifecycle lock");
    let Some(entry) = shared.entries.get_mut(&peer) else {
        tracing::debug!(%peer, "dial failed for untracked peer; dial path already reported");
        return;
    };
    // 监督者自己的重连在途（dialing）时结果由 Reconnected 报告，此处忽略
    if entry.machine.state() != ConnState::Connecting || entry.dialing {
        tracing::debug!(%peer, state = entry.machine.state().as_str(), "dial failure not attributable; ignored");
        return;
    }
    match entry.machine.transition(ConnState::Disconnected) {
        Ok(from) => emit_state(&handle.events, peer, from, ConnState::Disconnected),
        Err(err) => tracing::warn!(%peer, error = %err, "transition rejected on dial failure"),
    }
    shared.entries.remove(&peer);
    tracing::warn!(%peer, "dial failed; peer not tracked without a successful session");
}

/// 用户挂断：出册停止跟踪与重连（断开事件由 hangup 路径已发）。
pub(super) fn on_hung_up(handle: &LifecycleHandle, peer: PeerId) {
    let mut shared = handle.shared.lock().expect("lifecycle lock");
    let Some(entry) = shared.entries.get_mut(&peer) else {
        tracing::debug!(%peer, "hangup for untracked peer; ignored");
        return;
    };
    match entry.machine.transition(ConnState::Disconnected) {
        Ok(from) => emit_state(&handle.events, peer, from, ConnState::Disconnected),
        Err(err) => tracing::warn!(%peer, error = %err, "transition rejected on hangup"),
    }
    shared.entries.remove(&peer);
    tracing::info!(%peer, "peer hung up; lifecycle tracking removed");
}

/// 探测结果：命中清零；连续未命中达上限判离线——关死半开连接、发 PeerDown、
/// 进入退避重连。判离线是明确失败路径：事件 + warn 日志双留痕。
pub(super) fn on_probed(
    swarm: &std::sync::Weak<super::Swarm>,
    handle: &LifecycleHandle,
    peer: PeerId,
    ok: bool,
    detail: String,
) {
    let mut shared = handle.shared.lock().expect("lifecycle lock");
    let cfg = shared.cfg.clone();
    let Some(entry) = shared.entries.get_mut(&peer) else {
        tracing::debug!(%peer, "probe result for untracked peer; ignored");
        return;
    };
    entry.probing = false;
    if entry.machine.state() != ConnState::Connected {
        tracing::debug!(%peer, state = entry.machine.state().as_str(), "probe result on non-connected state; ignored");
        return;
    }
    if ok {
        if entry.misses > 0 {
            tracing::debug!(%peer, "probe recovered; miss streak cleared");
        }
        entry.misses = 0;
        // E8：探活命中是活跃度正信号（Dead 态由此恢复翻转）
        drop(shared);
        if let Some(strong) = swarm.upgrade() {
            strong
                .liveness
                .note_alive(peer, LivenessSource::Probe, unix_now());
        }
        return;
    }
    entry.misses += 1;
    tracing::warn!(%peer, misses = entry.misses, max = cfg.max_probe_misses, detail = %detail, "liveness probe missed");
    if entry.misses < cfg.max_probe_misses {
        return;
    }
    // 判离线：先出池并关死半开连接（serve 退出时 remove_if_same 不中，不重复发断开）
    if let Some(strong) = swarm.upgrade() {
        if let Some(mux) = strong.pool.take(&peer) {
            tracing::info!(%peer, "closing unresponsive connection after probe misses");
            // E8：判死关链归因 Error 档；探活死信号喂统一活跃度判定
            let _ = strong
                .lifecycle
                .events
                .send(LifecycleEvent::ConnectionClosed {
                    peer,
                    reason: CloseReason::Error,
                });
            strong
                .liveness
                .note_dead(peer, LivenessSource::Probe, unix_now());
            mux.close();
        }
    }
    entry.last_uptime = entry.up_since.map(|t| t.elapsed());
    if let Err(err) = entry.machine.transition(ConnState::BackingOff) {
        tracing::warn!(%peer, error = %err, "transition rejected on probe down");
        return;
    }
    emit_state(
        &handle.events,
        peer,
        ConnState::Connected,
        ConnState::BackingOff,
    );
    let reason = format!("probe missed {} consecutive probes: {detail}", entry.misses);
    let _ = handle
        .events
        .send(LifecycleEvent::PeerDown { peer, reason });
    schedule_retry(&cfg, peer, entry, "probe misses exhausted");
}

/// 重连拨号结果：成功兜底走 mark_connected（池命中竞速时 Connected 消息可能未发）；
/// 失败回到退避并排定下次（DialFailed 事件已由拨号路径发出）。
pub(super) fn on_reconnected(handle: &LifecycleHandle, peer: PeerId, ok: bool) {
    if ok {
        mark_connected(handle, peer);
        return;
    }
    let mut shared = handle.shared.lock().expect("lifecycle lock");
    let cfg = shared.cfg.clone();
    let Some(entry) = shared.entries.get_mut(&peer) else {
        tracing::debug!(%peer, "reconnect failure for untracked peer; ignored");
        return;
    };
    if entry.machine.state() != ConnState::Connecting {
        tracing::debug!(%peer, state = entry.machine.state().as_str(), "reconnect failure on non-connecting state; ignored");
        return;
    }
    entry.dialing = false;
    match entry.machine.transition(ConnState::BackingOff) {
        Ok(from) => {
            emit_state(&handle.events, peer, from, ConnState::BackingOff);
            tracing::warn!(%peer, attempts = entry.backoff.attempts(), "reconnect dial failed");
            schedule_retry(&cfg, peer, entry, "reconnect failed");
        }
        Err(err) => tracing::warn!(%peer, error = %err, "transition rejected on reconnect failure"),
    }
}
