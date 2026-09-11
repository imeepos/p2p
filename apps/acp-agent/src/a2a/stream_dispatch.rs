//! task 相 JSON-RPC 方法分派（tasks/get|send|cancel）与 create 建附路径。
//! 错误统一 -32000 + 机器码前缀；create 即转 working 并回 taskId 应答 + 状态通知。

use std::sync::Arc;

use a2a::{TaskCreateParams, TaskRequest, TaskResponse, TaskState};
use p2p::BoxedStream;
use serde_json::json;
use tokio::sync::mpsc;

use super::bridge::BridgeEvent;
use super::stream::{
    params_message, params_task_id, snapshot_of, status_notice, write_json, ERR_SERVER,
};
use super::task::{TaskHandle, TaskService};

pub(super) async fn dispatch(
    service: &Arc<TaskService>,
    current: Option<&Arc<TaskHandle>>,
    peer: &str,
    request: &TaskRequest,
) -> TaskResponse {
    let fail = |message: String| TaskResponse {
        jsonrpc: "2.0".into(),
        id: request.id,
        result: None,
        error: Some(a2a::TaskErrorBody {
            code: ERR_SERVER,
            message,
        }),
    };
    let ok = |result: serde_json::Value| TaskResponse {
        jsonrpc: "2.0".into(),
        id: request.id,
        result: Some(result),
        error: None,
    };
    match request.method.as_str() {
        "tasks/get" => {
            let Some(task_id) = params_task_id(&request.params) else {
                return fail("invalid-params: taskId required".into());
            };
            match service.snapshot_for(peer, &task_id) {
                Ok(task) => ok(serde_json::to_value(snapshot_of(&task)).unwrap_or_default()),
                Err(e) => fail(e.to_string()),
            }
        }
        "tasks/send" => {
            let Some(handle) = current else {
                return fail("no-task-on-stream: tasks/create first".into());
            };
            let task_id = handle.task_id();
            if params_task_id(&request.params).is_some_and(|t| t != task_id) {
                return fail("task-mismatch: stream carries another task".into());
            }
            let Some(message) = params_message(&request.params) else {
                return fail("invalid-params: message required".into());
            };
            match service.send(peer, &task_id, message).await {
                Ok(()) => ok(json!({ "taskId": task_id })),
                Err(e) => fail(e.to_string()),
            }
        }
        "tasks/cancel" => {
            let Some(handle) = current else {
                return fail("no-task-on-stream: tasks/create first".into());
            };
            let task_id = handle.task_id();
            match service.cancel(peer, &task_id).await {
                Ok(()) => ok(json!({ "taskId": task_id })),
                Err(e) => fail(e.to_string()),
            }
        }
        _ => fail(format!("unknown method {}", request.method)),
    }
}

pub(super) async fn create_and_attach(
    service: &Arc<TaskService>,
    peer: &str,
    is_owner: bool,
    request: &TaskRequest,
    stream: &mut BoxedStream,
) -> Result<(Arc<TaskHandle>, mpsc::Receiver<BridgeEvent>), String> {
    let params: TaskCreateParams = serde_json::from_value(request.params.clone())
        .map_err(|_| "invalid-params: agentId/message".to_owned())?;
    let handle = service
        .create(peer, is_owner, &params.agent_id, params.message)
        .await
        .map_err(|e| e.to_string())?;
    // spawn 成功即转 working（握手失败随后以 failed 状态回流，不静默）。
    let state = {
        let mut task = handle.task.lock().unwrap_or_else(|p| p.into_inner());
        task.transition(TaskState::Working)
            .map_err(|e| e.to_string())?;
        task.state
    };
    let reply = TaskResponse {
        jsonrpc: "2.0".into(),
        id: request.id,
        result: Some(json!({ "taskId": handle.task_id() })),
        error: None,
    };
    write_json(stream, &reply)
        .await
        .map_err(|e| e.to_string())?;
    write_json(stream, &status_notice(&handle.task_id(), state))
        .await
        .map_err(|e| e.to_string())?;
    let rx = handle
        .take_events()
        .ok_or_else(|| "task events already attached".to_owned())?;
    Ok((handle, rx))
}
