//! router 状态机（设计 §5/§6/§7）：单任务持有子进程 stdin 与输出面路由。
//! attach = 子进程输出透传 wire（权限瀑布拦截点）；detach = 续连窗口
//! （session/update 入环形缓存、权限请求无人值守 reject-once）；
//! 窗口过期或子进程崩溃走退出阶梯（stdin EOF -> 宽限 -> SIGKILL）。

use std::collections::VecDeque;
use std::io;
use std::sync::Arc;
use std::time::Instant;

use acp_common::PeerPolicy;
use serde_json::Value;
use tokio::process::{Child, ChildStdin};
use tokio::sync::mpsc::{Receiver, Sender};
use uuid::Uuid;

use crate::audit::{AuditEvent, AuditSink};
use crate::child::{Ctl, SlotBook};
use crate::config::AgentConfig;
use crate::permission;
use crate::reattach;

mod window;

pub(crate) struct RouterParams {
    pub peer_id: String,
    pub conn: String,
    pub grant: PeerPolicy,
    pub audit: Arc<dyn AuditSink>,
    pub config: AgentConfig,
    pub book: Arc<SlotBook>,
    pub ticket: Uuid,
}

/// 窗口期挂起的权限请求：超时即代答 reject-once（设计 §6 工具行 60s 上限）。
struct Outstanding {
    id: Value,
    deadline: Instant,
}

struct Router {
    params: RouterParams,
    stdin: ChildStdin,
    conn: String,
    sink: Option<Sender<Vec<u8>>>,
    cache: reattach::UpdateCache,
    outstanding: Vec<Outstanding>,
    window_deadline: Option<Instant>,
    replay_pending: bool,
    /// 首连 spawn 到 attach 之间的早期子进程输出：attach 时回放，绝不静默丢弃。
    premain: VecDeque<Vec<u8>>,
    ever_attached: bool,
}

enum Flow {
    Continue,
    Stop(Exit),
}

#[derive(Debug)]
enum Exit {
    Shutdown,
    WindowExpired,
    ChildGone,
    Guardrail(String),
}

enum Event {
    Child(Option<io::Result<Vec<u8>>>),
    Ctl(Option<Ctl>),
    Tick,
}

/// router 主循环：stdout 行、控制面、deadline 三路合流，单任务串行处理。
pub(crate) async fn run(
    params: RouterParams,
    child: Child,
    stdin: ChildStdin,
    mut lines: Receiver<io::Result<Vec<u8>>>,
    mut ctl: Receiver<Ctl>,
) {
    let mut router = Router::new(params, stdin);
    let exit = loop {
        let event = next_event(&router, &mut lines, &mut ctl).await;
        let flow = match event {
            Event::Child(Some(Ok(bytes))) => router.on_child_line(&bytes).await,
            Event::Child(Some(Err(err))) => Flow::Stop(Exit::Guardrail(err.to_string())),
            Event::Child(None) => Flow::Stop(Exit::ChildGone),
            Event::Ctl(Some(msg)) => router.on_ctl(msg).await,
            Event::Ctl(None) => Flow::Stop(Exit::Shutdown),
            Event::Tick => router.on_deadline().await,
        };
        if let Flow::Stop(exit) = flow {
            break exit;
        }
    };
    router.finish(child, exit).await;
}

async fn next_event(
    router: &Router,
    lines: &mut Receiver<io::Result<Vec<u8>>>,
    ctl: &mut Receiver<Ctl>,
) -> Event {
    let deadline = tokio_deadline(router.next_deadline());
    tokio::select! {
        line = lines.recv() => Event::Child(line),
        msg = ctl.recv() => Event::Ctl(msg),
        _ = tokio::time::sleep_until(deadline) => Event::Tick,
    }
}

fn tokio_deadline(deadline: Option<Instant>) -> tokio::time::Instant {
    deadline
        .map(tokio::time::Instant::from_std)
        .unwrap_or_else(|| tokio::time::Instant::now() + std::time::Duration::from_secs(3600))
}

impl Router {
    fn new(params: RouterParams, stdin: ChildStdin) -> Self {
        let conn = params.conn.clone();
        Self {
            params,
            stdin,
            conn,
            sink: None,
            cache: reattach::UpdateCache::new(),
            outstanding: Vec::new(),
            window_deadline: None,
            replay_pending: false,
            premain: VecDeque::new(),
            ever_attached: false,
        }
    }

    async fn on_child_line(&mut self, bytes: &[u8]) -> Flow {
        let Ok(root) = serde_json::from_slice::<Value>(bytes) else {
            return self.forward(bytes).await;
        };
        if let Some(req) = permission::classify(&root) {
            return self.on_permission(req, bytes).await;
        }
        // 首连 attach 前的 update 走 premain 缓冲；脱离窗口期的 update 才进环形缓存。
        if reattach::is_session_update(&root) && self.ever_attached && self.window_active() {
            self.cache_update(&root, bytes);
            return Flow::Continue;
        }
        self.forward(bytes).await
    }

    async fn on_ctl(&mut self, msg: Ctl) -> Flow {
        match msg {
            Ctl::Shutdown => Flow::Stop(Exit::Shutdown),
            Ctl::Detach => {
                self.enter_window().await;
                Flow::Continue
            }
            Ctl::Attach { sink, defer, conn } => {
                self.attach(sink, defer, conn).await;
                Flow::Continue
            }
            Ctl::ToChild(bytes) => self.on_client_line(bytes).await,
        }
    }

    async fn on_client_line(&mut self, bytes: Vec<u8>) -> Flow {
        if self.write_stdin(&bytes).await.is_err() {
            return Flow::Stop(Exit::ChildGone);
        }
        let Ok(root) = serde_json::from_slice::<Value>(&bytes) else {
            return Flow::Continue;
        };
        if self.replay_pending && reattach::is_method(&root, "initialize") {
            self.replay_now().await;
        }
        if reattach::is_response(&root) {
            let id = root.get("id").cloned().unwrap_or(Value::Null);
            self.settle(&id);
        }
        Flow::Continue
    }

    async fn on_deadline(&mut self) -> Flow {
        let now = Instant::now();
        let due: Vec<Value> = self
            .outstanding
            .iter()
            .filter(|o| o.deadline <= now)
            .map(|o| o.id.clone())
            .collect();
        for id in &due {
            let response = permission::rejected_response(id);
            if self.write_stdin(response.as_bytes()).await.is_err() {
                return Flow::Stop(Exit::ChildGone);
            }
            self.settle(id);
            self.audit_perm("timeout-rejected", &id.to_string());
        }
        if self.window_deadline.is_some_and(|d| d <= now) {
            self.params.audit.record(AuditEvent::WindowExpired {
                peer: self.params.peer_id.clone(),
                detail: format!("ticket={}", self.params.ticket),
            });
            return Flow::Stop(Exit::WindowExpired);
        }
        Flow::Continue
    }
}
