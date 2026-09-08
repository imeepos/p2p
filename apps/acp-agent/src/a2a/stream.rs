//! task 相流循环（a2a-over-p2p-design §5.2 JSON-RPC 2.0）：一条流承载一个任务
//! （Q10）。首帧 tasks/create（或只读 tasks/get 断线恢复）；桥事件转
//! tasks/message 与 tasks/status 通知；EOF = 客户端断流，活跃任务 cancel 后
//! 兜底收尾（孤儿进程不过夜）。单循环单写者，应答/通知无并发写竞争。

use std::io;
use std::sync::Arc;

use a2a::{
    Message, MessageNoticeParams, Part, Role, StatusParams, TaskErrorBody, TaskNotice,
    TaskRequest, TaskResponse, TaskSnapshot, TaskState,
};
use p2p::{BoxedStream, PeerId};
use p2p_protocol::{read_frame, write_frame};
use serde_json::json;
use tokio::sync::mpsc;

use super::bridge::BridgeEvent;
use super::task::{TaskHandle, TaskService};

/// 业务错误统一 JSON-RPC -32000，机器码进 message 前缀（gate-denied 等）。
const ERR_SERVER: i64 = -32000;

pub(super) async fn serve_task_stream(
    service: Arc<TaskService>,
    mut stream: BoxedStream,
    peer: PeerId,
    is_owner: bool,
) -> io::Result<()> {
    let peer_str = peer.to_string();
    let mut current: Option<(Arc<TaskHandle>, mpsc::Receiver<BridgeEvent>)> = None;
    loop {
        tokio::select! {
            biased;
            event = async {
                match current.as_mut() {
                    Some((_, rx)) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                let Some((handle, _)) = current.as_ref() else { continue };
                match event {
                    Some(BridgeEvent::Message { message_id, part }) => {
                        let notice = on_agent_message(handle, message_id, part);
                        write_json(&mut stream, &notice).await?;
                    }
                    Some(BridgeEvent::Done(state)) => {
                        finalize(&service, handle, state, &mut stream).await?;
                        current = None;
                    }
                    None => {}
                }
            }
            inbound = read_frame(&mut stream) => {
                let bytes = match to_bytes(inbound) {
                    Some(bytes) => bytes,
                    None => break, // EOF：收尾在循环外
                };
                let Ok(request) = serde_json::from_slice::<TaskRequest>(&bytes) else {
                    reply_err(&mut stream, None, -32700, "parse-error").await?;
                    continue;
                };
                if request.method == "tasks/create" {
                    if current.is_some() {
                        reply_err(&mut stream, request.id, ERR_SERVER, "one-task-per-stream").await?;
                        continue;
                    }
                    match create_and_attach(&service, &peer_str, is_owner, &request, &mut stream).await {
                        Ok(pair) => current = Some(pair),
                        Err(message) => reply_err(&mut stream, request.id, ERR_SERVER, &message).await?,
                    }
                    continue;
                }
                let reply = dispatch(&service, current.as_ref().map(|(h, _)| h), &peer_str, &request).await;
                write_json(&mut stream, &reply).await?;
            }
        }
    }
    if let Some((handle, _rx)) = current.take() {
        finish_detached(&service, handle).await;
    }
    Ok(())
}

async fn dispatch(
    service: &Arc<TaskService>,
    current: Option<&Arc<TaskHandle>>,
    peer: &str,
    request: &TaskRequest,
) -> TaskResponse {
    let fail = |message: String| TaskResponse {
        jsonrpc: "2.0".into(),
        id: request.id,
        result: None,
        error: Some(TaskErrorBody { code: ERR_SERVER, message }),
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
            let Some(task_id) = params_task_id(&request.params) else {
                return fail("invalid-params: taskId required".into());
            };
            if task_id != handle.task_id() {
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
            match service.cancel(peer, &handle.task_id()).await {
                Ok(()) => ok(json!({ "taskId": handle.task_id() })),
                Err(e) => fail(e.to_string()),
            }
        }
        _ => fail(format!("unknown method {}", request.method)),
    }
}

async fn create_and_attach(
    service: &Arc<TaskService>,
    peer: &str,
    is_owner: bool,
    request: &TaskRequest,
    stream: &mut BoxedStream,
) -> Result<(Arc<TaskHandle>, mpsc::Receiver<BridgeEvent>), String> {
    let params: a2a::TaskCreateParams = serde_json::from_value(request.params.clone())
        .map_err(|_| "invalid-params: agentId/message".to_owned())?;
    let handle = service
        .create(peer, is_owner, &params.agent_id, params.message)
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
    let rx = handle.take_events().ok_or("task events already attached")?;
    Ok((handle, rx))
}


/// agent chunk -> 簿记 + tasks/message 通知帧。
fn on_agent_message(handle: &Arc<TaskHandle>, message_id: String, part: Part) -> TaskNotice {
    let message = Message {
        role: Role::Agent,
        parts: vec![part],
    };
    if let Ok(mut task) = handle.task.lock() {
        let _ = task.append_agent(message.clone());
    }
    TaskNotice::Message {
        jsonrpc: "2.0".into(),
        params: MessageNoticeParams {
            task_id: handle.task_id(),
            message_id,
            message,
        },
    }
}

/// 终态回写 + status 通知 + 出簿（额度守卫随句柄销毁释放）。
async fn finalize(
    service: &Arc<TaskService>,
    handle: &Arc<TaskHandle>,
    state: TaskState,
    stream: &mut BoxedStream,
) -> io::Result<()> {
    let task_id = handle.task_id();
    if let Ok(mut task) = handle.task.lock() {
        let _ = task.transition(state);
    }
    write_json(stream, &status_notice(&task_id, state)).await?;
    service.remove(&task_id);
    Ok(())
}

/// 断流兜底：cancel 桥并消费 Done 事件完成状态回写与出簿。
async fn finish_detached(service: &Arc<TaskService>, handle: Arc<TaskHandle>) {
    let _ = service.cancel(&handle.peer, &handle.task_id()).await;
    if let Some(mut rx) = handle.take_events() {
        while let Some(event) = rx.recv().await {
            if let BridgeEvent::Done(state) = event {
                if let Ok(mut task) = handle.task.lock() {
                    let _ = task.transition(state);
                }
                break;
            }
        }
    }
    service.remove(&handle.task_id());
}

fn status_notice(task_id: &str, state: TaskState) -> TaskNotice {
    TaskNotice::Status {
        jsonrpc: "2.0".into(),
        params: StatusParams {
            task_id: task_id.to_owned(),
            state,
        },
    }
}

fn snapshot_of(task: &a2a::Task) -> TaskSnapshot {
    TaskSnapshot {
        task_id: task.task_id.clone(),
        agent_id: task.agent_id.clone(),
        state: task.state,
        messages: task.messages.clone(),
    }
}

fn params_task_id(params: &serde_json::Value) -> Option<String> {
    params.get("taskId").and_then(|v| v.as_str()).map(str::to_owned)
}

fn params_message(params: &serde_json::Value) -> Option<Message> {
    serde_json::from_value(params.get("message").cloned()?).ok()
}

fn to_bytes(inbound: io::Result<Vec<u8>>) -> Option<Vec<u8>> {
    match inbound {
        Ok(bytes) if bytes.is_empty() => None, // 对端关流
        Ok(bytes) => Some(bytes),
        Err(err) => {
            tracing::debug!(error = %err, "a2a task stream read failed");
            None
        }
    }
}

async fn write_json(stream: &mut BoxedStream, value: &impl serde::Serialize) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    write_frame(stream, &bytes).await
}

async fn reply_err(
    stream: &mut BoxedStream,
    id: Option<u64>,
    code: i64,
    message: &str,
) -> io::Result<()> {
    let reply = TaskResponse {
        jsonrpc: "2.0".into(),
        id,
        result: None,
        error: Some(TaskErrorBody {
            code,
            message: message.to_owned(),
        }),
    };
    write_json(stream, &reply).await
}
