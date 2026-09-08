//! task 状态机与 parts 类型（docs/design/a2a-over-p2p-design.md §5.2/§5.3）。
//! 状态迁移合法路径显式：submitted→working→completed|failed|cancelled；
//! submitted→rejected；非法迁移一律拒绝（防协议状态机错乱）。

use serde::{Deserialize, Serialize};

/// 每 task 累计上行输入上限（字节）：防经济 DoS（design §9 F7）。
pub const TASK_INPUT_CAP_BYTES: usize = 256 * 1024;
/// 每 peer 并发 task 流上限（design §5.2 资源门禁；宿主 TaskGate 消费）。
pub const TASKS_PER_PEER_MAX: usize = 4;
/// FilePart 单文件上限（字节）：防 16 MiB ACP 单行护栏溢出（design §9 F7）。
pub const FILE_PART_CAP_BYTES: usize = 4 * 1024 * 1024;

/// A2A Task 状态（裁剪：v1 不产生 input_required/auth_required，见设计 §5.2 负空间）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Submitted,
    Working,
    Completed,
    Failed,
    Cancelled,
    Rejected,
}

/// 非法状态迁移错误。
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("非法状态迁移 {from:?} → {to:?}")]
pub struct TransitionError {
    pub from: TaskState,
    pub to: TaskState,
}

/// 合法迁移表：终态（completed/failed/cancelled/rejected）不可再迁移。
pub fn allowed_transition(from: TaskState, to: TaskState) -> bool {
    matches!(
        (from, to),
        (TaskState::Submitted, TaskState::Working)
            | (TaskState::Submitted, TaskState::Rejected)
            | (TaskState::Working, TaskState::Completed)
            | (TaskState::Working, TaskState::Failed)
            | (TaskState::Working, TaskState::Cancelled)
    )
}

/// 消息角色。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Agent,
}

/// TextPart：文本消息。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextPart {
    pub text: String,
}

/// FilePart：附件（bytes base64，≤ FILE_PART_CAP_BYTES）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilePart {
    pub name: String,
    pub mime_type: String,
    pub bytes: String,
}

/// DataPart：结构化 JSON 数据（折叠信息卡渲染）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataPart {
    pub data: serde_json::Value,
}

/// A2A 标准三件 parts。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    Text(TextPart),
    File(FilePart),
    Data(DataPart),
}

impl Part {
    /// v1 上行仅 TextPart（设计 §5.3）：File/Data 上行显式拒绝，不静默。
    pub fn is_uploadable(&self) -> bool {
        matches!(self, Part::Text(_))
    }

    /// 上行输入字节估计：text 长度；file 按 base64 长度（防超限）。
    pub fn input_bytes(&self) -> usize {
        match self {
            Part::Text(t) => t.text.len(),
            Part::File(f) => f.bytes.len(),
            Part::Data(d) => serde_json::to_vec(d).map(|v| v.len()).unwrap_or(0),
        }
    }
}

/// 消息：role + parts（v1 一条消息限 1 个 part，多 part 留协议扩展）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub role: Role,
    pub parts: Vec<Part>,
}

impl Message {
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            parts: vec![Part::Text(TextPart { text: text.into() })],
        }
    }

    /// v1 上行校验：role=user、恰 1 个可上传 part、输入不超累计上限。
    pub fn validate_upload(&self, budget_left: usize) -> Result<(), TaskError> {
        if self.role != Role::User {
            return Err(TaskError::RoleNotUser);
        }
        if self.parts.len() != 1 {
            return Err(TaskError::PartCount);
        }
        let part = &self.parts[0];
        if !part.is_uploadable() {
            return Err(TaskError::PartNotUploadable);
        }
        if part.input_bytes() > budget_left {
            return Err(TaskError::InputOverCap);
        }
        Ok(())
    }
}

/// task 操作错误。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TaskError {
    #[error("v1 仅支持 role=user 上行")]
    RoleNotUser,
    #[error("v1 一条消息限 1 个 part")]
    PartCount,
    #[error("v1 上行仅 TextPart（File/Data 显式拒绝）")]
    PartNotUploadable,
    #[error("输入超过每 task 累计上限")]
    InputOverCap,
    #[error("非法状态迁移 {0}")]
    Transition(#[from] TransitionError),
    #[error("消息未按序追加（agent 消息须在 working 态）")]
    MessageOutOfOrder,
}

/// task 聚合：状态 + 消息序（宿主持有，快照供 tasks/get）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub task_id: String,
    pub agent_id: String,
    pub state: TaskState,
    pub messages: Vec<Message>,
    /// 上行累计输入字节（记账，tasks/get 快照不含）。
    pub input_used: usize,
}

impl Task {
    pub fn new(task_id: String, agent_id: String, first: Message) -> Self {
        let input_used = first.parts.iter().map(Part::input_bytes).sum::<usize>();
        Self {
            task_id,
            agent_id,
            state: TaskState::Submitted,
            messages: vec![first],
            input_used,
        }
    }

    /// 状态迁移：合法路径放行，非法拒绝并留错误。
    pub fn transition(&mut self, to: TaskState) -> Result<(), TaskError> {
        if !allowed_transition(self.state, to) {
            return Err(TransitionError {
                from: self.state,
                to,
            }
            .into());
        }
        self.state = to;
        Ok(())
    }

    /// 追加用户消息（续聊）：仅 working 态可追加（提交后、完成前）。
    pub fn append_user(&mut self, message: Message) -> Result<(), TaskError> {
        if self.state != TaskState::Working {
            return Err(TaskError::MessageOutOfOrder);
        }
        let budget = TASK_INPUT_CAP_BYTES.saturating_sub(self.input_used);
        message.validate_upload(budget)?;
        self.input_used += message.parts.iter().map(Part::input_bytes).sum::<usize>();
        self.messages.push(message);
        Ok(())
    }

    /// 追加 agent 消息（桥转写 agent_message_chunk）：仅 working 态。
    pub fn append_agent(&mut self, message: Message) -> Result<(), TaskError> {
        if self.state != TaskState::Working {
            return Err(TaskError::MessageOutOfOrder);
        }
        self.messages.push(message);
        Ok(())
    }
}
