//! 子进程槽位（设计 §5/§7）：进程归桥持有，票据绑定 PeerId；客户端断流不杀进程。
//! 本模块只管簿记、spawn 与 stdout 读取；状态机（attach/detach/窗口/退出阶梯）
//! 在 router 模块。router 结束时自行从簿记摘除并 reap 子进程。

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::process::ChildStdout;
use tokio::sync::mpsc;
use uuid::Uuid;

use acp_common::PeerPolicy;

use crate::audit::AuditSink;
use crate::config::AgentConfig;
use crate::router::{self, RouterParams};
use crate::subprocess;

/// 控制通道容量：会话侧行速受 wire 护栏约束，64 深度足够吸收抖动。
pub(crate) const CTL_CAP: usize = 64;
/// 子进程输出行通道容量：router 停止消费时 reader 背压阻塞，不积压无界缓冲。
const LINES_CAP: usize = 64;

/// 会话 -> router 的控制面。
pub(crate) enum Ctl {
    /// 客户端行（已过 mcpServers 改写）写子进程 stdin；响应行结算 outstanding。
    ToChild(Vec<u8>),
    /// 接管输出面；defer = 续连场景，待 initialize 过桥后再补放缓存。
    Attach {
        sink: mpsc::Sender<Vec<u8>>,
        defer: bool,
        conn: String,
    },
    /// 客户端断流：进入续连窗口。
    Detach,
    /// 立即退出阶梯（窗口过期 / 顶替 / 停机）。
    Shutdown,
}

#[derive(Clone)]
pub(crate) struct SlotHandle {
    pub peer: String,
    pub ticket: Uuid,
    pub ctl: mpsc::Sender<Ctl>,
}

/// 票据 -> 槽位簿记。跨 serve() 调用存活，挂在 SessionDeps 上。
pub struct SlotBook {
    slots: Mutex<HashMap<Uuid, SlotHandle>>,
    tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl SlotBook {
    pub fn new() -> Self {
        Self {
            slots: Mutex::new(HashMap::new()),
            tasks: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn track(&self, task: tokio::task::JoinHandle<()>) {
        self.tasks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(task);
    }

    /// 桥自身退出（设计 §7）：向全部活槽位广播 Shutdown，限时等待各 router
    /// 走完退出阶梯（stdin EOF -> 宽限 -> SIGKILL）。返回处理的槽位数。
    pub async fn shutdown_all(&self, wait: Duration) -> usize {
        let handles: Vec<SlotHandle> = self
            .slots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .drain()
            .map(|(_, handle)| handle)
            .collect();
        let count = handles.len();
        for handle in &handles {
            if let Err(err) = handle.ctl.try_send(Ctl::Shutdown) {
                tracing::warn!(peer = %handle.peer, error = %err, "shutdown broadcast failed");
            }
        }
        let tasks: Vec<tokio::task::JoinHandle<()>> = std::mem::take(
            &mut self
                .tasks
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
        let reap_all = async {
            for task in tasks {
                let _ = task.await;
            }
        };
        if tokio::time::timeout(wait, reap_all).await.is_err() {
            tracing::error!(
                slots = count,
                "shutdown wait timed out; kill_on_drop is last resort"
            );
        }
        count
    }

    pub(crate) fn insert(&self, handle: SlotHandle) {
        self.lock().insert(handle.ticket, handle);
    }

    /// 票据校验：存在且绑定同一 PeerId（票据防跨设备劫持）。
    pub(crate) fn validate(&self, ticket: &Uuid, peer: &str) -> Option<SlotHandle> {
        let slots = self.lock();
        slots
            .get(ticket)
            .filter(|handle| handle.peer == peer)
            .cloned()
    }

    /// 顶替：peer 无票据重连时杀掉遗留窗口槽位（孤儿进程不过夜）。
    /// 返回顶替数供审计。try_send 失败时 router 也会因窗口过期自行退出，
    /// 失败留 error 日志，不静默。
    pub(crate) fn supersede(&self, peer: &str) -> usize {
        let mut slots = self.lock();
        let stale: Vec<Uuid> = slots
            .values()
            .filter(|handle| handle.peer == peer)
            .map(|handle| handle.ticket)
            .collect();
        for ticket in &stale {
            if let Some(handle) = slots.remove(ticket) {
                if let Err(err) = handle.ctl.try_send(Ctl::Shutdown) {
                    tracing::error!(peer, %ticket, error = %err, "supersede shutdown send failed");
                }
            }
        }
        stale.len()
    }

    pub(crate) fn remove(&self, ticket: &Uuid) {
        self.lock().remove(ticket);
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<Uuid, SlotHandle>> {
        self.slots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Default for SlotBook {
    fn default() -> Self {
        Self::new()
    }
}

/// spawn 上下文：打包会话侧依赖，控制 spawn_slot 参数面。
pub(crate) struct SpawnCtx {
    pub config: AgentConfig,
    pub audit: Arc<dyn AuditSink>,
    pub book: Arc<SlotBook>,
    pub peer_id: String,
    pub conn: String,
    pub grant: PeerPolicy,
}

/// spawn 子进程并组装 router + reader；票据在此签发。
pub(crate) fn spawn_slot(
    ctx: SpawnCtx,
    cwd: Option<PathBuf>,
    stderr_log: PathBuf,
) -> io::Result<SlotHandle> {
    let SpawnCtx {
        config,
        audit,
        book,
        peer_id,
        conn,
        grant,
    } = ctx;
    let sub = subprocess::spawn(&config.command, stderr_log, cwd)?;
    let (ctl_tx, ctl_rx) = mpsc::channel(CTL_CAP);
    let (lines_tx, lines_rx) = mpsc::channel(LINES_CAP);
    let ticket = Uuid::new_v4();
    let handle = SlotHandle {
        peer: peer_id.clone(),
        ticket,
        ctl: ctl_tx,
    };
    book.insert(handle.clone());
    tokio::spawn(reader_task(sub.stdout, lines_tx));
    let params = RouterParams {
        peer_id,
        conn,
        grant,
        audit,
        config: config.clone(),
        book: book.clone(),
        ticket,
    };
    let task = tokio::spawn(router::run(params, sub.child, sub.stdin, lines_rx, ctl_rx));
    book.track(task);
    Ok(handle)
}

/// 子进程 stdout 逐行读取；行护栏击穿以 Err 传递，router 侧毒化退出。
async fn reader_task(stdout: ChildStdout, tx: mpsc::Sender<io::Result<Vec<u8>>>) {
    let mut stdout = tokio::io::BufReader::new(stdout);
    loop {
        let mut line = Vec::new();
        let verdict = match crate::pump::read_bounded_line(&mut stdout, &mut line).await {
            Ok(true) => break,
            Ok(false) => Ok(line),
            Err(err) => Err(err),
        };
        if tx.send(verdict).await.is_err() {
            return;
        }
    }
}

/// serve 返回即 Detach 的守卫：drop 走 try_send（同步上下文），失败留日志；
/// 即使丢失，router 的窗口过期仍是兜底退出路径。
pub(crate) struct DetachGuard {
    ctl: mpsc::Sender<Ctl>,
}

impl DetachGuard {
    pub(crate) fn new(ctl: mpsc::Sender<Ctl>) -> Self {
        Self { ctl }
    }
}

impl Drop for DetachGuard {
    fn drop(&mut self) {
        if let Err(err) = self.ctl.try_send(Ctl::Detach) {
            tracing::warn!(error = %err, "detach notify failed; router window may linger");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(ticket: Uuid, peer: &str) -> (SlotHandle, mpsc::Receiver<Ctl>) {
        let (tx, rx) = mpsc::channel(CTL_CAP);
        (
            SlotHandle {
                peer: peer.to_owned(),
                ticket,
                ctl: tx,
            },
            rx,
        )
    }

    #[tokio::test]
    async fn validate_rejects_cross_device_ticket() {
        let book = SlotBook::new();
        let ticket = Uuid::new_v4();
        let (handle, _rx) = slot(ticket, "peer-a");
        book.insert(handle);
        assert!(book.validate(&ticket, "peer-a").is_some());
        assert!(book.validate(&ticket, "peer-b").is_none());
    }

    #[tokio::test]
    async fn supersede_shuts_down_same_peer_slots() {
        let book = SlotBook::new();
        let (handle, mut rx) = slot(Uuid::new_v4(), "peer-a");
        book.insert(handle);
        let (other, mut other_rx) = slot(Uuid::new_v4(), "peer-b");
        book.insert(other);
        assert_eq!(book.supersede("peer-a"), 1);
        assert!(matches!(rx.recv().await, Some(Ctl::Shutdown)));
        assert!(other_rx.try_recv().is_err());
        assert!(book.validate(&Uuid::new_v4(), "peer-a").is_none());
    }
}
