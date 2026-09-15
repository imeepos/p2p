//! 会话注册表：按 PeerId 维护，同一 Peer 单活跃会话（控制/视频处理器共享）。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use p2p::PeerId;

/// 会话注册表：peer → 会话上下文。
#[derive(Default)]
pub struct HostSessions {
    inner: HashMap<PeerId, HostCtx>,
}

/// 单会话上下文：会话 id + 停止旗标（控制/视频任一侧退出即置位）。
pub struct HostCtx {
    pub session_id: String,
    pub stop: Arc<AtomicBool>,
}

impl HostSessions {
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// 登记会话；同 peer 已有活跃会话则拒绝。
    pub fn insert(&mut self, peer: PeerId, session_id: String) -> Result<(), String> {
        if self.inner.contains_key(&peer) {
            return Err("session already active for peer".into());
        }
        let stop = Arc::new(AtomicBool::new(false));
        self.inner.insert(peer, HostCtx { session_id, stop });
        Ok(())
    }

    pub fn get(&self, peer: &PeerId) -> Option<&HostCtx> {
        self.inner.get(peer)
    }

    /// 置位停止旗标（通知对侧退出）。
    pub fn signal_stop(&self, peer: &PeerId) {
        if let Some(ctx) = self.inner.get(peer) {
            ctx.stop.store(true, Ordering::Relaxed);
        }
    }

    /// 移除会话（幂等）。
    pub fn remove(&mut self, peer: &PeerId) -> Option<HostCtx> {
        self.inner.remove(peer)
    }
}
