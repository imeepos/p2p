//! task 句柄与操作错误（a2a-over-p2p-design §5.2）：单个在册任务的簿记面。
//! 终态后并发额度释放（release_quota）而快照留簿——tasks/get 断线恢复的
//! 权威终态来源，直至逐旧出簿（REGISTRY_MAX）。

use std::sync::Mutex;

use a2a::Task;
use tokio::sync::mpsc;

use super::bridge::{BridgeCmd, BridgeEvent};
use super::limits::TaskGuard;

/// create 路径错误：denied = 门禁（可见性/授权清单），cap = 限流。
#[derive(Debug, thiserror::Error)]
pub enum TaskOpError {
    #[error("not-found: agent 不存在或停用")]
    AgentUnknown,
    #[error("gate-denied: agent 不对请求方开放")]
    GateDenied,
    #[error("not-found: task 不存在或不属请求方")]
    TaskUnknown,
    #[error("task-cap: {0}")]
    Cap(&'static str),
    #[error("subprocess-failed: {0}")]
    Spawn(String),
    #[error("{0}")]
    State(#[from] a2a::TaskError),
}

/// 单个在册任务：a2a::Task 状态机 + 桥控制面 + 事件面 + 并发额度守卫。
/// 终态后额度释放（release_quota）而快照留簿（tasks/get 断线恢复，§5.2）。
pub struct TaskHandle {
    pub peer: String,
    pub task: Mutex<Task>,
    pub cmd_tx: mpsc::Sender<BridgeCmd>,
    /// 事件面单流消费：attach 即 take；无流时由 finish_detached 兜底。
    pub event_rx: Mutex<Option<mpsc::Receiver<BridgeEvent>>>,
    pub(crate) guard: Mutex<Option<TaskGuard>>,
}

impl TaskHandle {
    pub fn task_id(&self) -> String {
        self.task
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .task_id
            .clone()
    }

    /// 终态收尾：释放并发额度（守卫落体），快照仍留簿供 tasks/get。
    pub fn release_quota(&self) {
        self.guard
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
    }

    /// 取走事件面（每 task 仅一次，创建流独占）。
    pub fn take_events(&self) -> Option<mpsc::Receiver<BridgeEvent>> {
        self.event_rx
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}
