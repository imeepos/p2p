//! task 相服务（a2a-over-p2p-design §5.2/§9）：task 簿记 + 授权门禁 + 桥编排。
//! 1 task = 1 流 = 1 子进程（Q10）：create 即 spawn 专属子进程，终态（含断流
//! 取消）出簿释放并发额度；私有 agent 走授权清单默认拒绝；任务按创建者 PeerId
//! 鉴权（§9），get 只读可跨流，send/cancel 限本流当前任务。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use a2a::{Message, Task, TaskState, TASK_INPUT_CAP_BYTES};
use tokio::sync::mpsc;

use crate::audit::{AuditEvent, AuditSink};
use crate::config::AgentConfig;
use crate::jail;
use crate::workspaces::WorkspaceStore;

use super::agents::AgentStore;
use super::bridge::{BridgeCmd, BridgeEvent, BridgeParams};
use super::grants::GrantStore;
use super::limits::{TaskGate, TaskGuard};

/// 终态 task 留簿上限（tasks/get 快照可查；超出逐最旧终态）。
pub const REGISTRY_MAX: usize = 256;

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
pub struct TaskHandle {
    pub peer: String,
    pub task: Mutex<Task>,
    pub cmd_tx: mpsc::Sender<BridgeCmd>,
    /// 事件面单流消费：attach 即 take；无流时由 finish_detached 兜底。
    pub event_rx: Mutex<Option<mpsc::Receiver<BridgeEvent>>>,
    _guard: TaskGuard,
}

impl TaskHandle {
    pub fn task_id(&self) -> String {
        self.task
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .task_id
            .clone()
    }

    /// 取走事件面（每 task 仅一次，创建流独占）。
    pub fn take_events(&self) -> Option<mpsc::Receiver<BridgeEvent>> {
        self.event_rx
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

pub struct TaskService {
    pub(crate) config: AgentConfig,
    pub(crate) agents: Arc<AgentStore>,
    pub(crate) grants: Arc<GrantStore>,
    pub(crate) workspaces: Arc<WorkspaceStore>,
    pub(crate) audit: Arc<dyn AuditSink>,
    gate: Arc<TaskGate>,
    registry: Mutex<HashMap<String, Arc<TaskHandle>>>,
}

impl TaskService {
    pub fn new(
        config: AgentConfig,
        agents: Arc<AgentStore>,
        grants: Arc<GrantStore>,
        workspaces: Arc<WorkspaceStore>,
        audit: Arc<dyn AuditSink>,
    ) -> Self {
        Self {
            config,
            agents,
            grants,
            workspaces,
            audit,
            gate: Arc::new(TaskGate::new()),
            registry: Mutex::new(HashMap::new()),
        }
    }

    /// 可见性门禁（§6/§9）：local 仅 owner；public 全放行；private 授权清单内。
    pub fn authorize(&self, agent_id: &str, peer: &str, is_owner: bool) -> Result<(), TaskOpError> {
        let Some(def) = self.agents.get(agent_id) else {
            return Err(TaskOpError::AgentUnknown);
        };
        if !def.enabled {
            return Err(TaskOpError::AgentUnknown);
        }
        let allowed = match def.visibility {
            a2a::Visibility::Local => is_owner,
            a2a::Visibility::Public => true,
            a2a::Visibility::Private => is_owner || self.grants.is_granted(agent_id, peer),
        };
        if !allowed {
            self.audit.record(AuditEvent::A2aTaskDenied {
                peer: peer.to_owned(),
                detail: format!("agent {agent_id} visibility={:?}", def.visibility),
            });
            return Err(TaskOpError::GateDenied);
        }
        Ok(())
    }

    /// 建 task：校验 -> 门禁 -> 限流 -> spawn 桥 -> 入簿。返回句柄供流 attach。
    pub fn create(
        &self,
        peer: &str,
        is_owner: bool,
        agent_id: &str,
        message: Message,
    ) -> Result<Arc<TaskHandle>, TaskOpError> {
        self.authorize(agent_id, peer, is_owner)?;
        // v1 上行校验（role=user、单 TextPart、预算内）在纯库聚合上完成。
        message
            .clone()
            .validate_upload(TASK_INPUT_CAP_BYTES)
            .map_err(TaskOpError::State)?;
        let guard = TaskGuard::acquire(self.gate.clone(), peer)
            .map_err(|kind| TaskOpError::Cap(if kind == super::limits::TaskCapKind::PerPeer { "per-peer" } else { "total" }))?;
        let task_id = uuid::Uuid::new_v4().simple().to_string();
        let def = self.agents.get(agent_id).ok_or(TaskOpError::AgentUnknown)?;
        let visibility_local = def.visibility == a2a::Visibility::Local;
        let cwd = self.resolve_cwd(&def.agent_id, visibility_local);
        let (cmd_tx, event_rx) = super::bridge::spawn(BridgeParams {
            peer: peer.to_owned(),
            task_id: task_id.clone(),
            command: self.config.command.clone(),
            stderr_log: self.config.log_dir().join(format!("a2a-{task_id}.log")),
            cwd,
            grace: self.config.grace(),
            permission_timeout: self.config.permission_timeout(),
            visibility_local,
            audit: self.audit.clone(),
        })
        .map_err(|e| TaskOpError::Spawn(e.to_string()))?;
        let task = Task::new(task_id.clone(), agent_id.to_owned(), message);
        let handle = Arc::new(TaskHandle {
            peer: peer.to_owned(),
            task: Mutex::new(task),
            cmd_tx,
            event_rx: Mutex::new(Some(event_rx)),
            _guard: guard,
        });
        self.insert(handle.clone());
        Ok(handle)
    }

    /// tasks/get：同 peer 只读快照（可跨流，断线恢复语义）。
    pub fn snapshot_for(&self, peer: &str, task_id: &str) -> Result<Task, TaskOpError> {
        let handle = self.lookup(peer, task_id)?;
        let snapshot = handle
            .task
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        Ok(snapshot)
    }

    /// tasks/send：仅 working 态、预算内；转发桥队列。
    pub async fn send(
        &self,
        peer: &str,
        task_id: &str,
        message: Message,
    ) -> Result<(), TaskOpError> {
        let handle = self.lookup(peer, task_id)?;
        let text = {
            let mut task = handle.task.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            task.append_user(message).map_err(TaskOpError::State)?;
            match &task.messages.last().expect("just pushed").parts[0] {
                a2a::Part::Text(t) => t.text.clone(),
                _ => unreachable!("append_user 只放行 TextPart"),
            }
        };
        handle
            .cmd_tx
            .send(BridgeCmd::Prompt(text))
            .await
            .map_err(|_| TaskOpError::Spawn("bridge gone".into()))?;
        Ok(())
    }

    /// tasks/cancel：转发桥（quiesce + 审计在桥内），状态经 Done 事件回写。
    pub async fn cancel(&self, peer: &str, task_id: &str) -> Result<(), TaskOpError> {
        let handle = self.lookup(peer, task_id)?;
        handle
            .cmd_tx
            .send(BridgeCmd::Cancel)
            .await
            .map_err(|_| TaskOpError::Spawn("bridge gone".into()))?;
        Ok(())
    }

    pub fn remove(&self, task_id: &str) {
        self.registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(task_id);
    }

    fn lookup(&self, peer: &str, task_id: &str) -> Result<Arc<TaskHandle>, TaskOpError> {
        let registry = self
            .registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        registry
            .get(task_id)
            .filter(|h| h.peer == peer)
            .cloned()
            .ok_or(TaskOpError::TaskUnknown)
    }

    fn insert(&self, handle: Arc<TaskHandle>) {
        let mut registry = self
            .registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if registry.len() >= REGISTRY_MAX {
            // 逐最旧终态（终态 task 不再产事件，只服务快照）。
            let oldest = registry
                .iter()
                .find(|(_, h)| {
                    matches!(
                        h.task.lock().unwrap_or_else(|p| p.into_inner()).state,
                        TaskState::Completed
                            | TaskState::Failed
                            | TaskState::Cancelled
                            | TaskState::Rejected
                    )
                })
                .map(|(id, _)| id.clone());
            if let Some(id) = oldest {
                registry.remove(&id);
            }
        }
        let task_id = handle
            .task
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .task_id
            .clone();
        registry.insert(task_id, handle);
    }

    /// §9 矩阵 cwd 列：local（owner 全权）继承桥 cwd；public/private 强制 sandbox。
    fn resolve_cwd(&self, agent_id: &str, visibility_local: bool) -> Option<PathBuf> {
        if visibility_local {
            return None;
        }
        match jail::resolve(
            &self.config,
            &self.workspaces,
            acp_common::Scope::Sandbox,
            &format!("agent-{agent_id}"),
            None,
        ) {
            Ok(cwd) => cwd,
            Err(err) => {
                tracing::warn!(agent = %agent_id, error = %err, "a2a sandbox jail resolve failed; cwd inherited");
                None
            }
        }
    }
}
