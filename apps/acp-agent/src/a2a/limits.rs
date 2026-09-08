//! task 相限流（a2a-over-p2p-design §5.2/§9）：每 peer 并发 task ≤4、宿主总量
//! 可配。acquire 临界区无 await；guard drop 即释放，终态/断流路径不漏记账。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use a2a::TASKS_PER_PEER_MAX;

/// 宿主级 task 总量上限（跨 peer；防整机子进程耗尽）。
pub const TASKS_TOTAL_MAX: usize = 32;

#[derive(Default)]
struct GateState {
    per_peer: HashMap<String, usize>,
    total: usize,
}

#[derive(Default)]
pub struct TaskGate {
    state: Mutex<GateState>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TaskCapKind {
    PerPeer,
    Total,
}

impl TaskGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// 超限返回上限种类；调用方据此回 JSON-RPC 错误并审计。
    pub fn try_acquire(&self, peer: &str) -> Result<(), TaskCapKind> {
        let mut state = self.lock();
        if state.total >= TASKS_TOTAL_MAX {
            return Err(TaskCapKind::Total);
        }
        let held = state.per_peer.get(peer).copied().unwrap_or(0);
        if held >= TASKS_PER_PEER_MAX {
            return Err(TaskCapKind::PerPeer);
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

/// task 存续期持锁守卫：drop 即释放。
pub struct TaskGuard {
    gate: Arc<TaskGate>,
    peer: String,
}

impl TaskGuard {
    pub fn acquire(gate: Arc<TaskGate>, peer: &str) -> Result<Self, TaskCapKind> {
        gate.try_acquire(peer)?;
        Ok(Self {
            gate,
            peer: peer.to_owned(),
        })
    }
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        self.gate.release(&self.peer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_peer_cap_enforced_and_released() {
        let gate = Arc::new(TaskGate::new());
        let guards: Vec<_> = (0..TASKS_PER_PEER_MAX)
            .map(|_| TaskGuard::acquire(gate.clone(), "p1").unwrap())
            .collect();
        assert_eq!(
            TaskGate::try_acquire(&gate, "p1").unwrap_err(),
            TaskCapKind::PerPeer
        );
        drop(guards);
        TaskGuard::acquire(gate.clone(), "p1").unwrap();
    }

    #[test]
    fn release_frees_slot() {
        let gate = Arc::new(TaskGate::new());
        let guard = TaskGuard::acquire(gate.clone(), "p1").unwrap();
        drop(guard);
        TaskGuard::acquire(gate.clone(), "p1").unwrap();
    }

    #[test]
    fn total_cap_enforced() {
        let gate = Arc::new(TaskGate::new());
        let guards: Vec<_> = (0..TASKS_TOTAL_MAX)
            .map(|i| TaskGuard::acquire(gate.clone(), &format!("p{i}")).unwrap())
            .collect();
        assert_eq!(
            TaskGate::try_acquire(&gate, "overflow").unwrap_err(),
            TaskCapKind::Total
        );
        drop(guards);
        TaskGuard::acquire(gate.clone(), "overflow").unwrap();
    }
}
