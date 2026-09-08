//! task 相流循环（a2a-over-p2p-design §5.2 JSON-RPC 2.0）：一条流承载一个任务
//! （Q10）。首帧 tasks/create（或只读 tasks/get 断线恢复；首帧由 handler 嗅探后
//! 传入）；桥事件转 tasks/message 与 tasks/status 通知；EOF = 客户端断流，活跃
//! 任务 cancel 后兜底收尾（孤儿进程不过夜）。单循环单写者，无并发写竞争。

use std::io;
use std::sync::Arc;

use a2a::{
    Message, MessageNoticeParams, Part, Role, StatusParams, TaskErrorBody, TaskNotice,
    TaskRequest, TaskResponse, TaskSnapshot, TaskState,
};
use p2p::{BoxedStream, PeerId};
use p2p_protocol::{read_frame, write_frame};
use tokio::sync::mpsc;

use super::bridge::BridgeEvent;
use super::stream_dispatch::{create_and_attach, dispatch};
use super::task::{TaskHandle, TaskService};

/// 业务错误统一 JSON-RPC -32000，机器码进 message 前缀（gate-denied 等）。
pub(super) const ERR_SERVER: i64 = -32000;

type Attached = Option<(Arc<TaskHandle>, mpsc::Receiver<BridgeEvent>)>;

pub(super) async fn serve_task_stream(
    service: Arc<TaskService>,
    mut stream: BoxedStream,
    peer: PeerId,
    is_owner: bool,
    first: Vec<u8>,
) -> io::Result<()> {
    let peer_str = peer.to_string();
    let mut current: Attached = None;
    if handle_frame(&service, &mut stream, &peer_str, is_owner, &mut current, &first).await? {
        return Ok(()); // 对端在首帧后即关流
    }
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
                if handle_frame(&service, &mut stream, &peer_str, is_owner, &mut current, &bytes)
                    .await?
                {
                    break;
                }
            }
        }
    }
    if let Some((handle, _rx)) = current.take() {
        finish_detached(&service, handle).await;
    }
    Ok(())
}

/// 处理一条入站请求；返回 true 表示对端关流（EOF 帧形态）。
async fn handle_frame(
    service: &Arc<TaskService>,
    stream: &mut BoxedStream,
    peer: &str,
    is_owner: bool,
    current: &mut Attached,
    bytes: &[u8],
) -> io::Result<bool> {
    if bytes.is_empty() {
        return Ok(true);
    }
    let Ok(request) = serde_json::from_slice::<TaskRequest>(bytes) else {
        reply_err(stream, None, -32700, "parse-error").await?;
        return Ok(false);
    };
    if request.method == "tasks/create" {
        if current.is_some() {
            reply_err(stream, request.id, ERR_SERVER, "one-task-per-stream").await?;
            return Ok(false);
        }
        match create_and_attach(service, peer, is_owner, &request, stream).await {
            Ok(pair) => *current = Some(pair),
            Err(message) => reply_err(stream, request.id, ERR_SERVER, &message).await?,
        }
        return Ok(false);
    }
    let reply = dispatch(service, current.as_ref().map(|(h, _)| h), peer, &request).await;
    write_json(stream, &reply).await.map(|_| false)
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
    service.finalize_task(handle);
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

pub(super) fn status_notice(task_id: &str, state: TaskState) -> TaskNotice {
    TaskNotice::Status {
        jsonrpc: "2.0".into(),
        params: StatusParams {
            task_id: task_id.to_owned(),
            state,
        },
    }
}

pub(super) fn snapshot_of(task: &a2a::Task) -> TaskSnapshot {
    TaskSnapshot {
        task_id: task.task_id.clone(),
        agent_id: task.agent_id.clone(),
        state: task.state,
        messages: task.messages.clone(),
    }
}

pub(super) fn params_task_id(params: &serde_json::Value) -> Option<String> {
    params.get("taskId").and_then(|v| v.as_str()).map(str::to_owned)
}

pub(super) fn params_message(params: &serde_json::Value) -> Option<Message> {
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

pub(super) async fn write_json(stream: &mut BoxedStream, value: &impl serde::Serialize) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    write_frame(stream, &bytes).await
}

pub(super) async fn reply_err(
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
