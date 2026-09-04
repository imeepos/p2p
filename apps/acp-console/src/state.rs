//! 连接状态机（需求 C）：connecting / online / reattach-window / offline。
//! 每次迁移落 tracing 日志 + stdout 事件行；快照经 watch 通道供 status 端点查询。

use serde::Serialize;
use tokio::sync::watch;

use crate::out;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConnPhase {
    Offline,
    Connecting,
    Online,
    ReattachWindow,
}

/// 状态快照：status 端点与 stdout 事件共用形状（GUI 波依赖，README 为契约权威）。
#[derive(Clone, Debug, Serialize)]
pub struct StateSnapshot {
    pub phase: ConnPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conn: Option<String>,
    /// 当前相位的进入时刻（unix 毫秒）。
    pub since_unix_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 状态中枢：单活跃连接语义（设计 §7 每 peer 并发连接 = 1 的骨架收敛）。
pub struct StatusHub {
    tx: watch::Sender<StateSnapshot>,
}

impl Default for StatusHub {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusHub {
    pub fn new() -> Self {
        let (tx, _) = watch::channel(StateSnapshot {
            phase: ConnPhase::Offline,
            peer: None,
            conn: None,
            since_unix_ms: now_unix_ms(),
            detail: None,
        });
        Self { tx }
    }

    pub fn snapshot(&self) -> StateSnapshot {
        self.tx.borrow().clone()
    }

    /// 订阅后续迁移（显式等待场景，避免轮询）。
    pub fn subscribe(&self) -> watch::Receiver<StateSnapshot> {
        self.tx.subscribe()
    }

    /// 状态迁移：日志 + stdout 事件 + watch 更新；无订阅者不构成错误。
    pub fn transition(
        &self,
        phase: ConnPhase,
        peer: Option<String>,
        conn: Option<String>,
        detail: Option<String>,
    ) {
        let prev = self.tx.borrow().phase;
        let snap = StateSnapshot {
            phase,
            peer,
            conn,
            since_unix_ms: now_unix_ms(),
            detail,
        };
        tracing::info!(
            from = ?prev,
            to = ?snap.phase,
            peer = ?snap.peer,
            detail = ?snap.detail,
            "conn state transition"
        );
        out::event("state", &snap);
        let _ = self.tx.send(snap);
    }
}
