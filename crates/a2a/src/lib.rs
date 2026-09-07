//! A2A over P2P 纯库（docs/design/a2a-over-p2p-design.md §4/§5）：零网络零进程，
//! 只承载模型/帧/簿/状态机。宿主接线在 apps/acp-agent；本 crate 依赖仅
//! serde/serde_json/bs58/p2p-identity（零网络依赖，非零依赖）。

pub mod book;
pub mod card;
pub mod frame;
pub mod task;

#[cfg(test)]
mod card_tests;
#[cfg(test)]
mod task_tests;

/// 协议 ID（wire-protocol §3：/命名空间/名字/版本，JSON 编码）。
/// 自 crate 定义（llm-share-offer 先例），wire-protocol §3.2 表登记。
pub const PROTOCOL_ID: &str = "/a2a/1";

/// 公开卡片 rendezvous namespace（design §7.1）：签名覆盖，与发现租户隔离。
pub const AGENT_NAMESPACE: &str = "a2a/agents/1";

/// card 相版本。
pub const CARD_FRAME_VERSION: u8 = 1;

pub use book::{AgentBook, BookError, BOOK_MAX_ENTRIES, ISSUED_AT_MAX_SKEW_SECS};
pub use card::{
    AgentCapabilities, AgentCard, AgentKey, AgentSkill, CardError, SignedCard, Visibility,
    AGENT_ID_MAX_CHARS, SKILLS_MAX, TTL_DEFAULT_SECS, TTL_MAX_SECS,
};
pub use frame::{
    CardFrame, MessageNoticeParams, StatusParams, TaskCreateParams, TaskErrorBody, TaskIdParams,
    TaskNotice, TaskRequest, TaskResponse, TaskSendParams, TaskSnapshot,
};
pub use task::{
    allowed_transition, DataPart, FilePart, Message, Part, Role, Task, TaskError, TaskState,
    TextPart, FILE_PART_CAP_BYTES, TASK_INPUT_CAP_BYTES,
};
