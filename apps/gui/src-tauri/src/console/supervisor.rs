//! 监督循环（gui-contract.md §15）：spawn acp-console → 解析 stdout ready 行 →
//! 异常退出按指数退避自动重启 → 连续失败达上限转 failed 停止重试并留 lastError；
//! stop 位触发收尾终止子进程（RunEvent::Exit 调 SupervisorHandle::shutdown）。
//! stderr 只进日志不参与协议。

use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};
use tokio::sync::{watch, Notify};

use super::spec::{backoff_for, Limits, SpawnSpec, STOP_POLL};
use super::status::{parse_line, AcpConsolePhase, AcpConsoleStatus};

/// 单轮 spawn + 收割的结论。
#[derive(Debug)]
enum Attempt {
    Stop,
    Crash(String),
    SpawnFailure(String),
}

/// 监督共享态：句柄与监督任务共同持有。
pub(super) struct Supervisor {
    spec: SpawnSpec,
    limits: Limits,
    status_tx: watch::Sender<AcpConsoleStatus>,
    stop: AtomicBool,
    wake: Notify,
    child: Mutex<Option<Child>>,
    consecutive: AtomicU32,
    restarts: AtomicU64,
    /// 首次达失败上限时的原因（failed 终态留痕用）。
    max_hit_reason: Mutex<Option<String>>,
}

impl Supervisor {
    pub(super) fn new(
        spec: SpawnSpec,
        limits: Limits,
        status_tx: watch::Sender<AcpConsoleStatus>,
    ) -> Self {
        Self {
            spec,
            limits,
            status_tx,
            stop: AtomicBool::new(false),
            wake: Notify::new(),
            child: Mutex::new(None),
            consecutive: AtomicU32::new(0),
            restarts: AtomicU64::new(0),
            max_hit_reason: Mutex::new(None),
        }
    }

    fn stop_flag(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// 逐行消费 stdout 直到 EOF 或观察到 stop（轮询臂兜底孤儿持管道的场景）。
    async fn pump(&self, stdout: ChildStdout) -> bool {
        let mut lines = BufReader::new(stdout).lines();
        loop {
            tokio::select! {
                line = lines.next_line() => match line {
                    Ok(Some(line)) => self.on_line(&line),
                    Ok(None) => return self.stop_flag(),
                    Err(e) => {
                        tracing::error!("acp-console stdout 读取失败: {e}");
                        return self.stop_flag();
                    }
                },
                _ = tokio::time::sleep(STOP_POLL) => {
                    if self.stop_flag() {
                        return true;
                    }
                }
            }
        }
    }

    /// 单行分发：ready 装配并清零连续失败；其他协议行忽略；非法行计失败观测。
    fn on_line(&self, line: &str) {
        match parse_line(line) {
            Ok(Some(ready)) => {
                self.consecutive.store(0, Ordering::SeqCst);
                self.status_tx.send_modify(|s| s.apply_ready(&ready));
                tracing::info!(
                    "acp-console 就绪（累计自动重启 {} 次）",
                    self.restarts.load(Ordering::SeqCst)
                );
            }
            Ok(None) => {}
            Err(reason) => {
                if self.register_failure(&reason) {
                    tracing::error!("acp-console 解析失败达上限，终止子进程并转 failed");
                    self.kill_child();
                }
            }
        }
    }

    /// spawn 子进程并消费其输出至退出（锁槽与收尾互斥，杜绝收尾竞态漏杀）。
    async fn attempt(&self) -> Attempt {
        if self.stop_flag() {
            return Attempt::Stop;
        }
        let (stdout, stderr) = match self.spawn_locked() {
            Ok(pipes) => pipes,
            Err(a) => return a,
        };
        tokio::spawn(drain_stderr(stderr));
        let stop_seen = self.pump(stdout).await;
        let exit_desc = self.reap().await;
        if stop_seen || self.stop_flag() {
            Attempt::Stop
        } else {
            Attempt::Crash(exit_desc)
        }
    }

    /// 锁槽 spawn（同步函数：守卫不跨 await）；stdout/stderr 均 piped 并入槽。
    fn spawn_locked(&self) -> Result<(ChildStdout, ChildStderr), Attempt> {
        let mut slot = self
            .child
            .lock()
            .map_err(|e| Attempt::SpawnFailure(format!("子进程槽锁中毒: {e}")))?;
        if self.stop_flag() {
            return Err(Attempt::Stop);
        }
        let mut cmd = Command::new(&self.spec.bin);
        cmd.args(&self.spec.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd
            .spawn()
            .map_err(|e| Attempt::SpawnFailure(format!("启动 acp-console 失败: {e}")))?;
        let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            return Err(Attempt::SpawnFailure(
                "acp-console 子进程 stdio 管道缺失".into(),
            ));
        };
        *slot = Some(child);
        tracing::info!("acp-console 子进程已启动: {}", self.spec.bin.display());
        Ok((stdout, stderr))
    }

    /// 收割子进程拿退出描述；槽空（已被退出收尾接管）时如实说明。
    async fn reap(&self) -> String {
        let child = self.child.lock().ok().and_then(|mut s| s.take());
        let Some(mut child) = child else {
            return "子进程已被退出收尾接管".into();
        };
        match child.wait().await {
            Ok(status) => format!("acp-console 进程退出（{status}）"),
            Err(e) => format!("等待 acp-console 退出失败: {e}"),
        }
    }

    /// 指数退避等待；stop 或收尾唤醒时提前返回 false（应停止）。
    async fn backoff(&self) -> bool {
        let delay = backoff_for(&self.limits, self.consecutive.load(Ordering::SeqCst));
        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
            _ = self.wake.notified() => {}
        }
        !self.stop_flag()
    }

    /// 收尾三连（句柄层调用）：停机位 + 杀子进程 + 唤醒退避等待。
    pub(super) fn shutdown_now(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.kill_child();
        self.wake.notify_waiters();
    }

    /// 当前子进程 pid（测试观测 Exit 收尾用）。
    #[cfg(test)]
    pub(super) fn child_pid(&self) -> Option<u32> {
        let slot = self.child.lock().ok()?;
        slot.as_ref().and_then(Child::id)
    }

    /// 同步终止在跑子进程（start_kill 只发信号不阻塞）；无子进程时为 no-op。
    fn kill_child(&self) {
        if let Ok(mut slot) = self.child.lock() {
            if let Some(child) = slot.as_mut() {
                if let Err(e) = child.start_kill() {
                    tracing::error!("acp-console 子进程终止失败: {e}");
                }
            }
        }
    }

    /// 失败观测 +1（崩溃或解析失败）；返回是否已达连续失败上限。
    /// 首次达上限的原因即时捕获，供 failed 终态留痕（后续失败不覆盖）。
    fn register_failure(&self, reason: &str) -> bool {
        let count = self.consecutive.fetch_add(1, Ordering::SeqCst) + 1;
        let max_hit = count >= self.limits.max_consecutive_failures;
        if max_hit {
            match self.max_hit_reason.lock() {
                Ok(mut slot) => {
                    if slot.is_none() {
                        *slot = Some(reason.to_string());
                    }
                }
                Err(e) => tracing::error!("acp-console 监督状态锁中毒: {e}"),
            }
        }
        tracing::warn!(count, "acp-console 失败观测: {reason}");
        max_hit
    }

    /// 重启轮标记：restarts +1，phase=restarting，连接面清空，lastError 留因。
    fn mark_restarting(&self, reason: &str) {
        let restarts = self.restarts.fetch_add(1, Ordering::SeqCst) + 1;
        self.status_tx
            .send_modify(|s| s.mark_down(AcpConsolePhase::Restarting, restarts, reason));
    }

    /// stopped 终态：清连接面，保留 restarts/lastError 历史。
    fn finish_stopped(&self) {
        self.status_tx.send_modify(|s| s.mark_stopped());
    }

    /// failed 终态：留首次达上限的原因并停止重启。
    fn finish_failed(&self) {
        let (restarts, reason) = {
            let stored = self
                .max_hit_reason
                .lock()
                .ok()
                .and_then(|slot| slot.clone());
            let snapshot = self.status_tx.borrow();
            (
                snapshot.restarts,
                stored
                    .or(snapshot.last_error.clone())
                    .unwrap_or_else(|| "连续失败达上限".into()),
            )
        };
        self.status_tx
            .send_modify(|s| s.mark_down(AcpConsolePhase::Failed, restarts, &reason));
        tracing::error!(
            "acp-console 监督转 failed（连续失败达 {} 次），停止重启: {reason}",
            self.limits.max_consecutive_failures
        );
    }
}

/// 监督主循环：spawn → 收割 → 失败判定 → 退避重启 / 终态退出。
pub(super) async fn run(sup: Arc<Supervisor>) {
    loop {
        if sup.stop_flag() {
            sup.finish_stopped();
            return;
        }
        // 每轮 spawn 前把 phase 归 starting（首轮与重启轮一致，face 保持清空）
        sup.status_tx
            .send_modify(|s| s.phase = AcpConsolePhase::Starting);
        match sup.attempt().await {
            Attempt::Stop => {
                sup.finish_stopped();
                return;
            }
            Attempt::Crash(reason) | Attempt::SpawnFailure(reason) => {
                if sup.register_failure(&reason) {
                    sup.finish_failed();
                    return;
                }
                sup.mark_restarting(&reason);
                if !sup.backoff().await {
                    sup.finish_stopped();
                    return;
                }
            }
        }
    }
}

/// stderr 只进日志不参与协议（契约 §15）；行级留痕，读取中断记告警后结束。
async fn drain_stderr(stderr: ChildStderr) {
    let mut lines = BufReader::new(stderr).lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => tracing::info!(target: "acp-console", "stderr: {line}"),
            Ok(None) => return,
            Err(e) => {
                tracing::warn!(target: "acp-console", "stderr 读取中断: {e}");
                return;
            }
        }
    }
}
