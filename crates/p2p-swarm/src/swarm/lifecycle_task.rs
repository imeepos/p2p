//! 对端生命周期监督·事件循环（E6）：单任务消息驱动 + 定时 tick。
//!
//! 探测与重连拨号均为阻塞操作，一律派生任务执行并经消息回报结果，
//! 监督循环本身只做状态裁决与排程，永不为一次探测/拨号阻塞。
//! Swarm 引用持 Weak：所有用户引用消失即退出，不阻止 Swarm 释放。

use std::sync::{Arc, Weak};
use std::time::Duration;

use p2p_identity::PeerId;
use tokio::sync::mpsc;

use super::lifecycle::{LifecycleHandle, LifecycleMsg};
use super::ping;
use super::Swarm;
use crate::lifecycle::ConnState;

/// 无到期项时的睡眠上限；醒来仅重算最近期限，无忙等。
const MAX_SLEEP: Duration = Duration::from_secs(3600);

/// 装配期入口：enabled 时启动监督任务（Swarm 结构体装配完成后调用）。
pub(super) fn start_supervisor(swarm: &Arc<Swarm>, rx: mpsc::Receiver<LifecycleMsg>) {
    if !swarm.lifecycle.enabled {
        tracing::info!("peer lifecycle disabled; supervisor not started");
        return;
    }
    let cfg = swarm.lifecycle.cfg();
    tokio::spawn(supervisor(
        Arc::downgrade(swarm),
        rx,
        swarm.lifecycle.clone(),
    ));
    tracing::info!(
        probe_interval = ?cfg.probe_interval,
        probe_timeout = ?cfg.probe_timeout,
        max_probe_misses = cfg.max_probe_misses,
        reconnect_base = ?cfg.reconnect_base,
        reconnect_max = ?cfg.reconnect_max,
        jitter = cfg.reconnect_jitter,
        reset_min_uptime = ?cfg.reset_min_uptime,
        "peer lifecycle supervisor started"
    );
}

async fn supervisor(
    swarm: Weak<Swarm>,
    mut rx: mpsc::Receiver<LifecycleMsg>,
    handle: LifecycleHandle,
) {
    let mut shutdown = match swarm.upgrade() {
        Some(strong) => strong.shutdown_rx.clone(),
        None => return,
    };
    loop {
        let deadline = next_deadline(&handle);
        tokio::select! {
            _ = shutdown.changed() => break,
            _ = tokio::time::sleep_until(deadline) => tick(&swarm, &handle).await,
            msg = rx.recv() => match msg {
                Some(msg) => super::lifecycle_handlers::handle_msg(&handle, &swarm, msg),
                None => break,
            },
        }
        if swarm.upgrade().is_none() {
            tracing::debug!("swarm released; lifecycle supervisor exiting");
            break;
        }
    }
}

/// 最近到期点：Connected 的探测期限与 BackingOff 的重连期限取最小。
fn next_deadline(handle: &LifecycleHandle) -> tokio::time::Instant {
    let shared = handle.shared.lock().expect("lifecycle lock");
    let mut next = tokio::time::Instant::now() + MAX_SLEEP;
    for entry in shared.entries.values() {
        match entry.machine.state() {
            ConnState::Connected if !entry.probing => {
                if let Some(at) = entry.next_probe_at {
                    next = next.min(at);
                }
            }
            ConnState::BackingOff if !entry.dialing => {
                if let Some(at) = entry.reconnect_at {
                    next = next.min(at);
                }
            }
            _ => {}
        }
    }
    next
}

/// 到期处理：派发到期的探测与重连；状态翻转在锁内完成，网络动作在锁外派生。
async fn tick(swarm: &Weak<Swarm>, handle: &LifecycleHandle) {
    let now = tokio::time::Instant::now();
    let probes = due_probes(handle, now);
    for peer in probes {
        let cfg = handle.cfg();
        tokio::spawn(run_probe(
            swarm.clone(),
            peer,
            cfg.probe_timeout,
            handle.tx.clone(),
        ));
    }
    let retries = due_retries(handle, now);
    for peer in retries {
        tokio::spawn(run_reconnect(swarm.clone(), peer, handle.tx.clone()));
    }
}

fn due_probes(handle: &LifecycleHandle, now: tokio::time::Instant) -> Vec<PeerId> {
    let mut shared = handle.shared.lock().expect("lifecycle lock");
    let interval = shared.cfg.probe_interval;
    let mut due = Vec::new();
    for (peer, entry) in shared.entries.iter_mut() {
        if entry.machine.state() == ConnState::Connected
            && !entry.probing
            && entry.next_probe_at.is_some_and(|at| at <= now)
        {
            entry.probing = true;
            entry.next_probe_at = Some(now + interval);
            due.push(*peer);
        }
    }
    due
}

fn due_retries(handle: &LifecycleHandle, now: tokio::time::Instant) -> Vec<PeerId> {
    let mut shared = handle.shared.lock().expect("lifecycle lock");
    let mut due = Vec::new();
    for (peer, entry) in shared.entries.iter_mut() {
        if entry.machine.state() == ConnState::BackingOff
            && !entry.dialing
            && entry.reconnect_at.is_some_and(|at| at <= now)
        {
            // 退避到期即拨：BackingOff→Connecting（结果由 Reconnected 回报）
            entry.dialing = true;
            entry.scheduled = None;
            match entry.machine.transition(ConnState::Connecting) {
                Ok(from) => {
                    super::lifecycle::emit_state(&handle.events, *peer, from, ConnState::Connecting)
                }
                Err(err) => {
                    tracing::warn!(peer = %peer, error = %err, "transition rejected on retry due");
                    entry.dialing = false;
                    continue;
                }
            }
            due.push(*peer);
        }
    }
    due
}

/// 探测任务：只测在册连接（绝不因探测触发拨号）；结果带原因回报。
async fn run_probe(
    swarm: Weak<Swarm>,
    peer: PeerId,
    timeout: Duration,
    tx: mpsc::Sender<LifecycleMsg>,
) {
    let Some(strong) = swarm.upgrade() else {
        return;
    };
    let result = match strong.pool.get(&peer) {
        Some(mux) => ping::probe_once(&mux, timeout).await,
        None => Err("no pooled connection".to_string()),
    };
    let (ok, detail) = match result {
        Ok(()) => (true, String::new()),
        Err(detail) => (false, detail),
    };
    if let Err(err) = tx.send(LifecycleMsg::Probed { peer, ok, detail }).await {
        tracing::warn!(%peer, error = %err, "lifecycle closed; probe result dropped");
    }
}

/// 重连任务：走 swarm.connect 幂等降级链（直连→打洞→中继），结果回报监督者。
async fn run_reconnect(swarm: Weak<Swarm>, peer: PeerId, tx: mpsc::Sender<LifecycleMsg>) {
    let Some(strong) = swarm.upgrade() else {
        return;
    };
    tracing::debug!(%peer, "reconnect dial starting");
    let ok = strong.connect(peer).await.is_ok();
    if let Err(err) = tx.send(LifecycleMsg::Reconnected { peer, ok }).await {
        tracing::warn!(%peer, error = %err, "lifecycle closed; reconnect result dropped");
    }
}
