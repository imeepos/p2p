//! /a2a/1 帧编解码（docs/design/a2a-over-p2p-design.md §5）：card 相请求-响应帧 +
//! task 相 JSON-RPC 2.0 帧。纯 serde 类型 + roundtrip 单测，宿主负责收发接线。

use serde::{Deserialize, Serialize};

use crate::card::SignedCard;
use crate::task::{Message, TaskState};

/// card 相帧：带 v 版本与 id 关联（id=0 为服务端推送通知，无需应答）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CardFrame {
    /// C→S 拉取可见卡片全集。
    List { v: u8, id: u64 },
    /// C→S 单卡查询。
    Get { v: u8, id: u64, agent_id: String },
    /// C→S 订阅变更（幂等：重复订阅返回当前快照）。
    Subscribe { v: u8, id: u64 },
    /// S→C 批量应答（id 回显）。
    Cards {
        v: u8,
        id: u64,
        cards: Vec<SignedCard>,
    },
    /// S→C subscribe 应答。
    Ok { v: u8, id: u64, subscribed: bool },
    /// S→C 变更推送（新增/更新入 cards；移除入 removed，键形 hostPeer/agentId）。
    Push {
        v: u8,
        id: u64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        cards: Vec<SignedCard>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        removed: Vec<String>,
    },
    /// S→C 错误应答（id 回显；code: not-found|denied|bad-card）。
    Error {
        v: u8,
        id: u64,
        code: String,
        message: String,
    },
    /// C→S 邀请请求（owner 生成签名凭证邀请帧）。
    InviteRequest {
        v: u8,
        id: u64,
        /// 邀请帧 JSON（Signed<InvitePayload>）。
        invite: serde_json::Value,
    },
    /// S→C 邀请应答（成功返回邀请帧 JSON，失败返回 error）。
    InviteResponse {
        v: u8,
        id: u64,
        /// 邀请帧 JSON（成功时）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        invite: Option<serde_json::Value>,
        /// 错误码（失败时）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        /// 错误消息（失败时）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    /// C→S 邀请回执提交（invitee 签名回执）。
    InviteReceipt {
        v: u8,
        id: u64,
        /// 回执 JSON（Signed<ReceiptPayload>）。
        receipt: serde_json::Value,
    },
    /// S→C 邀请回执应答（成功/失败）。
    InviteReceiptResponse {
        v: u8,
        id: u64,
        /// 是否成功。
        ok: bool,
        /// 错误消息（失败时）。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
}

/// task 相请求参数：tasks/create。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCreateParams {
    pub agent_id: String,
    pub message: Message,
}

/// task 相请求参数：tasks/send。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSendParams {
    pub task_id: String,
    pub message: Message,
}

/// task 相请求参数：tasks/get / tasks/cancel。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskIdParams {
    pub task_id: String,
}

/// task 相请求（JSON-RPC 2.0）：method + id 关联。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRequest {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    pub method: String,
    pub params: serde_json::Value,
}

/// task 相应答（JSON-RPC 2.0）：result 或 error 二选一（id 回显）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponse {
    pub jsonrpc: String,
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TaskErrorBody>,
}

/// JSON-RPC 错误体（A2A 业务码经 message 承载）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskErrorBody {
    pub code: i64,
    pub message: String,
}

/// task 相通知（无 id）：状态变更与 agent 消息。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum TaskNotice {
    #[serde(rename = "tasks/status")]
    Status {
        jsonrpc: String,
        params: StatusParams,
    },
    #[serde(rename = "tasks/message")]
    Message {
        jsonrpc: String,
        params: MessageNoticeParams,
    },
}

/// tasks/status 通知参数。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusParams {
    pub task_id: String,
    pub state: TaskState,
}

/// tasks/message 通知参数：messageId 供客户端去重（流式多帧同 taskId）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageNoticeParams {
    pub task_id: String,
    pub message_id: String,
    pub message: Message,
}

/// tasks/get 快照结果：state + messages 全量（断线恢复权威终态）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub task_id: String,
    pub agent_id: String,
    pub state: TaskState,
    pub messages: Vec<Message>,
}

impl TaskRequest {
    pub fn new(method: &str, id: u64, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id: Some(id),
            method: method.into(),
            params,
        }
    }

    pub fn create(agent_id: &str, message: Message, id: u64) -> Self {
        Self::new(
            "tasks/create",
            id,
            serde_json::json!({ "agentId": agent_id, "message": message }),
        )
    }

    pub fn send(task_id: &str, message: Message, id: u64) -> Self {
        Self::new(
            "tasks/send",
            id,
            serde_json::json!({ "taskId": task_id, "message": message }),
        )
    }

    pub fn get(task_id: &str, id: u64) -> Self {
        Self::new("tasks/get", id, serde_json::json!({ "taskId": task_id }))
    }

    pub fn cancel(task_id: &str, id: u64) -> Self {
        Self::new("tasks/cancel", id, serde_json::json!({ "taskId": task_id }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip<T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug>(
        v: &T,
    ) -> T {
        let bytes = serde_json::to_vec(v).expect("serialize");
        serde_json::from_slice(&bytes).expect("deserialize")
    }

    #[test]
    fn card_frame_roundtrip() {
        let frames = vec![
            CardFrame::List { v: 1, id: 1 },
            CardFrame::Get {
                v: 1,
                id: 2,
                agent_id: "code-review".into(),
            },
            CardFrame::Subscribe { v: 1, id: 3 },
            CardFrame::Cards {
                v: 1,
                id: 1,
                cards: vec![],
            },
            CardFrame::Ok {
                v: 1,
                id: 3,
                subscribed: true,
            },
            CardFrame::Push {
                v: 1,
                id: 0,
                cards: vec![],
                removed: vec![],
            },
            CardFrame::Error {
                v: 1,
                id: 2,
                code: "not-found".into(),
                message: "agent missing".into(),
            },
        ];
        for frame in &frames {
            let got = roundtrip(frame);
            assert_eq!(&got, frame);
        }
    }

    #[test]
    fn task_request_roundtrip() {
        let req = TaskRequest::create("code-review", Message::user_text("review this PR"), 1);
        let got = roundtrip(&req);
        assert_eq!(got.method, "tasks/create");
        assert_eq!(got.id, Some(1));
    }
}
