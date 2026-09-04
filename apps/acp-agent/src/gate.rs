//! 资源门禁（设计 §7）：每 peer 并发连接 1、连接总数可配。acquire/release 同步锁，
//! 临界区无 await；拒绝返回 (ErrorCode, 上限种类) 供 wire denied 与审计共用。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use acp_common::consts::MAX_CONNS_PER_PEER;
use acp_common::error::ErrorCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateLimits {
    pub per_peer: u32,
    pub total: u32,
}

impl GateLimits {
    pub fn from_config(total: u32) -> Self {
        Self {
            per_peer: MAX_CONNS_PER_PEER,
            total: total.max(1),
        }
    }
}

#[derive(Default)]
struct GateState {
    per_peer: HashMap<String, u32>,
    total: u32,
}

#[derive(Default)]
pub struct ConnGate {
    state: Mutex<GateState>,
}

impl ConnGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// 超限返回 (码, 上限种类)；码进 denied 帧，种类进审计。
    pub fn try_acquire(
        &self,
        peer: &str,
        limits: GateLimits,
    ) -> Result<(), (ErrorCode, &'static str)> {
        let mut state = self.lock();
        if state.total >= limits.total {
            return Err((ErrorCode::ConnCapReached { cap: limits.total }, "total"));
        }
        let held = state.per_peer.get(peer).copied().unwrap_or(0);
        if held >= limits.per_peer {
            return Err((
                ErrorCode::ConnCapReached {
                    cap: limits.per_peer,
                },
                "per-peer",
            ));
        }
        state.total += 1;
        *state.per_peer.entry(peer.to_owned()).or_insert(0) += 1;
        Ok(())
    }

    pub fn release(&self, peer: &str) {
        let mut state = self.lock();
        state.total = state.total.saturating_sub(1);
        if let Some(held) = state.per_peer.get_mut(peer) {
            *held = held.saturating_sub(1);
            if *held == 0 {
                state.per_peer.remove(peer);
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, GateState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// 会话期持锁守卫：drop 即释放，拒绝/断流/子进程退出路径不漏记账。
pub struct ConnGuard {
    gate: Arc<ConnGate>,
    peer: String,
}

impl ConnGuard {
    pub fn acquire(
        gate: Arc<ConnGate>,
        peer: &str,
        limits: GateLimits,
    ) -> Result<Self, (ErrorCode, &'static str)> {
        gate.try_acquire(peer, limits)?;
        Ok(Self {
            gate,
            peer: peer.to_owned(),
        })
    }
}

impl Drop for ConnGuard {
    fn drop(&mut self) {
        self.gate.release(&self.peer);
    }
}
