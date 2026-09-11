//! task⇄ACP 桥主循环（a2a-over-p2p-design §2/Q10）：每 task 专属子进程，桥内嵌
//! ACP client——握手后按 FIFO prompt 队列逐条 session/prompt；chunk 事件经
//! bridge_chunk 映射；权限请求经 §9 矩阵桥内闭环（不出 wire，见 bridge_perm 注）。
//! cancel = 子进程 quiesce（stdin EOF → 宽限 → SIGKILL）。

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use a2a::{Part, TaskState};
use serde_json::{json, Value};
use tokio::io::BufReader;
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::mpsc;

use crate::audit::{AuditEvent, AuditSink};
use crate::subprocess;

use super::bridge_chunk::{emit_chunk, is_chunk_update};
use super::bridge_io::{handshake, read_line, write_request};

/// 宿主 -> 桥控制面。
pub enum BridgeCmd {
    /// 追加一轮 prompt（v1 上行仅 TextPart，text 已由服务层校验）。
    Prompt(String),
    /// 取消：子进程 quiesce + cancelled 终态。
    Cancel,
}

/// 桥 -> 服务层事件（转 wire 帧）。
pub enum BridgeEvent {
    Message { message_id: String, part: Part },
    Done(TaskState),
}

/// 桥参数面：peer/task 供审计定位，visibility_local 决定 §9 矩阵列。
pub struct BridgeParams {
    pub peer: String,
    pub task_id: String,
    pub command: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub stderr_log: PathBuf,
    pub grace: Duration,
    pub permission_timeout: Duration,
    pub visibility_local: bool,
    pub audit: Arc<dyn AuditSink>,
}

/// prompt 状态：id 计数、FIFO 队列、在途 prompt 与挂起权限请求。
pub(crate) struct PromptCtx {
    pub stdin: ChildStdin,
    next_id: u64,
    pub queue: VecDeque<String>,
    pub prompt_id: Option<Value>,
    pub outstanding: Vec<Outstanding>,
}

pub(crate) struct Outstanding {
    pub id: Value,
    pub deadline: Instant,
}

impl PromptCtx {
    pub fn new(stdin: ChildStdin) -> Self {
        Self {
            stdin,
            next_id: 1,
            queue: VecDeque::new(),
            prompt_id: None,
            outstanding: Vec::new(),
        }
    }

    pub fn take_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

/// 启动桥：spawn 失败同步上抛（服务层回 SubprocessFailed）；循环任务运行至终态。
pub fn spawn(
    params: BridgeParams,
) -> Result<(mpsc::Sender<BridgeCmd>, mpsc::Receiver<BridgeEvent>), std::io::Error> {
    let sub = subprocess::spawn(
        &params.command,
        params.stderr_log.clone(),
        params.cwd.clone(),
    )?;
    let (cmd_tx, cmd_rx) = mpsc::channel::<BridgeCmd>(16);
    let (event_tx, event_rx) = mpsc::channel::<BridgeEvent>(64);
    tokio::spawn(run(
        params, sub.child, sub.stdin, sub.stdout, cmd_rx, event_tx,
    ));
    Ok((cmd_tx, event_rx))
}

async fn run(
    params: BridgeParams,
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    mut cmd_rx: mpsc::Receiver<BridgeCmd>,
    event_tx: mpsc::Sender<BridgeEvent>,
) {
    let mut reader = BufReader::new(stdout);
    let mut ctx = PromptCtx::new(stdin);
    let mut terminal = match handshake(&mut ctx, &mut reader, &params).await {
        Ok(()) => None,
        Err(err) => {
            params.audit.record(AuditEvent::SubprocessExit {
                peer: params.peer.clone(),
                conn: params.task_id.clone(),
                detail: format!("bridge handshake failed: {err}"),
            });
            Some(TaskState::Failed)
        }
    };
    while terminal.is_none() {
        let deadline = tokio::time::Instant::from_std(
            ctx.outstanding
                .iter()
                .map(|o| o.deadline)
                .min()
                .unwrap_or_else(|| Instant::now() + Duration::from_secs(3600)),
        );
        let event = tokio::select! {
            cmd = cmd_rx.recv() => Ev::Cmd(cmd),
            line = read_line(&mut reader) => Ev::Child(line),
            _ = tokio::time::sleep_until(deadline) => Ev::Tick,
        };
        terminal = match event {
            Ev::Cmd(Some(BridgeCmd::Cancel)) => Some(TaskState::Cancelled),
            Ev::Cmd(Some(BridgeCmd::Prompt(text))) => {
                ctx.queue.push_back(text);
                if ctx.prompt_id.is_none() {
                    start_prompt(&mut ctx, &params).await;
                }
                None
            }
            Ev::Cmd(None) => Some(TaskState::Failed),
            Ev::Child(Err(err)) => child_exit(&params, format!("guardrail: {err}")),
            Ev::Child(Ok(None)) => child_exit(&params, "child stdout eof".into()),
            Ev::Child(Ok(Some(line))) => on_child_line(&params, &mut ctx, &line, &event_tx).await,
            Ev::Tick => {
                super::bridge_perm::on_tick(&params, &mut ctx).await;
                None
            }
        };
    }
    if terminal == Some(TaskState::Cancelled) {
        // quiesce：stdin EOF 通知收尾，宽限后 SIGKILL（Q10 拍板 + 设计 §5.2）。
        drop(ctx.stdin);
        let detail = subprocess::reap(child, params.grace).await;
        params.audit.record(AuditEvent::A2aTaskCancelled {
            peer: params.peer.clone(),
            detail: format!("task {} quiesce: {detail}", params.task_id),
        });
    }
    let _ = event_tx
        .send(BridgeEvent::Done(terminal.unwrap_or(TaskState::Failed)))
        .await;
}

enum Ev {
    Cmd(Option<BridgeCmd>),
    Child(std::io::Result<Option<Vec<u8>>>),
    Tick,
}

fn child_exit(params: &BridgeParams, detail: String) -> Option<TaskState> {
    params.audit.record(AuditEvent::SubprocessExit {
        peer: params.peer.clone(),
        conn: params.task_id.clone(),
        detail,
    });
    Some(TaskState::Failed)
}

async fn start_prompt(ctx: &mut PromptCtx, params: &BridgeParams) {
    let Some(text) = ctx.queue.pop_front() else {
        return;
    };
    let id = ctx.take_id();
    ctx.prompt_id = Some(json!(id));
    if let Err(err) = write_request(
        &mut ctx.stdin,
        id,
        "session/prompt",
        json!({ "prompt": [ { "type": "text", "text": text } ] }),
    )
    .await
    {
        params.audit.record(AuditEvent::SubprocessExit {
            peer: params.peer.clone(),
            conn: params.task_id.clone(),
            detail: format!("prompt write failed: {err}"),
        });
    }
}

async fn on_child_line(
    params: &BridgeParams,
    ctx: &mut PromptCtx,
    raw: &[u8],
    event_tx: &mpsc::Sender<BridgeEvent>,
) -> Option<TaskState> {
    let Ok(root) = serde_json::from_slice::<Value>(raw) else {
        tracing::debug!(task = %params.task_id, "child non-json line ignored");
        return None;
    };
    if let Some(req) = crate::permission::classify(&root) {
        super::bridge_perm::answer_permission(params, ctx, req).await;
        return None;
    }
    if is_chunk_update(&root) {
        return emit_chunk(params, &root, event_tx).await;
    }
    if matches!(&ctx.prompt_id, Some(pid) if root.get("id").is_some_and(|v| v == pid)) {
        // prompt 结算（stopReason 语义：v1 任何应答即回合完成，文本已流式送达）
        ctx.prompt_id = None;
        if !ctx.queue.is_empty() {
            start_prompt(ctx, params).await;
            return None; // 续轮：仍在 working
        }
        return Some(TaskState::Completed);
    }
    // 权限应答结算（v1 无 GUI 通道不达；到达即移除，避免超时重复代答）
    ctx.outstanding.retain(|o| root.get("id") != Some(&o.id));
    None
}
