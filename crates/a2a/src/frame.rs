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
            serde_json::to_value(TaskCreateParams {
                agent_id: agent_id.into(),
                message,
            })
            .expect("params serializable"),
        )
    }

    pub fn send(task_id: &str, message: Message, id: u64) -> Self {
        Self::new(
            "tasks/send",
            id,
            serde_json::to_value(TaskSendParams {
                task_id: task_id.into(),
                message,
            })
            .expect("params serializable"),
        )
    }

    pub fn get(task_id: &str, id: u64) -> Self {
        Self::new(
            "tasks/get",
            id,
            serde_json::to_value(TaskIdParams {
                task_id: task_id.into(),
            })
            .expect("params serializable"),
        )
    }

    pub fn cancel(task_id: &str, id: u64) -> Self {
        Self::new(
            "tasks/cancel",
            id,
            serde_json::to_value(TaskIdParams {
                task_id: task_id.into(),
            })
            .expect("params serializable"),
        )
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
                removed: vec!["peer/a".into()],
            },
            CardFrame::Error {
                v: 1,
                id: 2,
                code: "not-found".into(),
                message: "no such agent".into(),
            },
        ];
        for f in &frames {
            assert_eq!(roundtrip(f), *f);
        }
    }

    #[test]
    fn task_request_roundtrip() {
        let req = TaskRequest::create("code-review", Message::user_text("你好"), 1);
        let back = roundtrip(&req);
        assert_eq!(back.method, "tasks/create");
        assert_eq!(back.id, Some(1));
        assert_eq!(back.jsonrpc, "2.0");
        let send = TaskRequest::send("t-1", Message::user_text("继续"), 2);
        assert_eq!(roundtrip(&send).method, "tasks/send");
        let get = TaskRequest::get("t-1", 3);
        assert_eq!(roundtrip(&get).method, "tasks/get");
        let cancel = TaskRequest::cancel("t-1", 4);
        assert_eq!(roundtrip(&cancel).method, "tasks/cancel");
    }

    #[test]
    fn task_response_roundtrip() {
        let ok = TaskResponse {
            jsonrpc: "2.0".into(),
            id: Some(1),
            result: Some(serde_json::json!({ "taskId": "t-1" })),
            error: None,
        };
        assert_eq!(roundtrip(&ok).result, ok.result);
        let err = TaskResponse {
            jsonrpc: "2.0".into(),
            id: Some(2),
            result: None,
            error: Some(TaskErrorBody {
                code: -32602,
                message: "bad params".into(),
            }),
        };
        assert_eq!(roundtrip(&err).error, err.error);
    }

    #[test]
    fn task_notice_roundtrip() {
        let status = TaskNotice::Status {
            jsonrpc: "2.0".into(),
            params: StatusParams {
                task_id: "t-1".into(),
                state: TaskState::Working,
            },
        };
        let back = roundtrip(&status);
        assert!(
            matches!(back, TaskNotice::Status { params, .. } if params.state == TaskState::Working)
        );
        let msg = TaskNotice::Message {
            jsonrpc: "2.0".into(),
            params: MessageNoticeParams {
                task_id: "t-1".into(),
                message_id: "m-1".into(),
                message: Message::user_text("hi"),
            },
        };
        let back = roundtrip(&msg);
        assert!(matches!(back, TaskNotice::Message { params, .. } if params.message_id == "m-1"));
    }

    #[test]
    fn snapshot_roundtrip() {
        let snap = TaskSnapshot {
            task_id: "t-1".into(),
            agent_id: "code-review".into(),
            state: TaskState::Completed,
            messages: vec![Message::user_text("你好")],
        };
        assert_eq!(roundtrip(&snap), snap);
    }
}
