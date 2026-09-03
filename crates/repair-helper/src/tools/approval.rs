//! 审批回调适配：把 repair-enforce Approval 状态机接到 helper 执行面（T23b）。
//!
//! P0b 语义（remote-support-plan.md §3.4）：write/danger 调用挂起等人工审批，
//! 60s 超时 = 拒绝（APPROVAL_TIMEOUT 编译期固定、不可放行）；时钟经 [Clock] 注入
//! 供测试推进（超时判定可测）。断线语义（§3.7）：审批期间对端断流无人应答，
//! 由通道决定——本批空队列即无人应答，60s 后超时拒绝。
//!
//! [drive_approval] 把状态机放到阻塞线程执行（Approval::run 为忙轮询，直接跑会
//! 卡 async 执行器）；裁决映射 Approved=放行 / Denied=拒 / Timeout=拒。
//! [QueueApprover] 是非阻塞队列通道：外部喂裁决，poll 只取队首；
//! [spawn_line_approver] 提供 P0b 行式输入（approve/deny 行）。MCP stdio 宿主下
//! stdin 已被协议占用，生产审批通道（托盘）由后续轮接线，行式实现供 CLI 演练。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use repair_enforce::approval::{Approval, ApprovalVerdict, Approver, Clock};

/// 墙钟：相对启动时刻的流逝时长（生产缺省实现）。
#[derive(Debug)]
pub struct WallClock {
    start: std::time::Instant,
}

impl WallClock {
    pub fn new() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }
}

impl Default for WallClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for WallClock {
    fn now(&self) -> std::time::Duration {
        self.start.elapsed()
    }
}

/// 脚本化时钟（测试）：外部推进时间，确定性驱动超时判定。
pub struct ScriptedClock {
    now: Mutex<std::time::Duration>,
}

impl ScriptedClock {
    pub fn new(now: std::time::Duration) -> Self {
        Self {
            now: Mutex::new(now),
        }
    }

    pub fn advance(&self, d: std::time::Duration) {
        if let Ok(mut slot) = self.now.lock() {
            *slot += d;
        }
    }
}

impl Clock for ScriptedClock {
    fn now(&self) -> std::time::Duration {
        self.now.lock().map(|s| *s).unwrap_or_default()
    }
}

/// 非阻塞审批通道：外部 [push] 裁决（行式输入/测试脚本），poll 取队首。
#[derive(Clone, Default)]
pub struct QueueApprover {
    queue: Arc<Mutex<VecDeque<ApprovalVerdict>>>,
}

impl QueueApprover {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, verdict: ApprovalVerdict) {
        if let Ok(mut q) = self.queue.lock() {
            q.push_back(verdict);
        }
    }
}

impl Approver for QueueApprover {
    fn poll(&mut self) -> Option<ApprovalVerdict> {
        self.queue.lock().ok().and_then(|mut q| q.pop_front())
    }
}

/// 行式审批输入：逐行读 approve/deny（大小写不敏感；空行/未知输入忽略）。
pub async fn spawn_line_approver<R>(reader: R) -> QueueApprover
where
    R: tokio::io::AsyncBufRead + Unpin + Send + 'static,
{
    use tokio::io::AsyncBufReadExt;
    let approver = QueueApprover::new();
    let sink = approver.clone();
    tokio::spawn(async move {
        let mut lines = reader.lines();
        loop {
            let Ok(Some(line)) = lines.next_line().await else {
                break;
            };
            let verdict = match line.trim().to_ascii_lowercase().as_str() {
                "approve" | "y" | "yes" => Some(ApprovalVerdict::Approved),
                "deny" | "n" | "no" => Some(ApprovalVerdict::Denied),
                _ => None,
            };
            if let Some(v) = verdict {
                sink.push(v);
            }
        }
    });
    approver
}

/// 驱动审批状态机（阻塞线程内忙轮询，防卡执行器）；join 失败按 Timeout 保守拒。
pub async fn drive_approval(
    clock: Arc<dyn Clock + Send + Sync>,
    approver: Arc<Mutex<Box<dyn Approver + Send>>>,
) -> ApprovalVerdict {
    tokio::task::spawn_blocking(move || {
        let mut guard = match approver.lock() {
            Ok(g) => g,
            Err(_) => {
                tracing::error!("approval channel lock poisoned; treating as denial");
                return ApprovalVerdict::Timeout;
            }
        };
        let clk = ClockShim(&*clock);
        let mut appr = ApproverShim(&mut **guard);
        let mut approval = Approval::open(&clk);
        approval.run(&clk, &mut appr)
    })
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "approval task panicked; treating as denial");
        ApprovalVerdict::Timeout
    })
    .unwrap_or(ApprovalVerdict::Timeout)
}

/// Sized 桥接：Approval::run 的 C 参数要求具体类型。
struct ClockShim<'a>(&'a (dyn Clock + Send + Sync));

impl Clock for ClockShim<'_> {
    fn now(&self) -> std::time::Duration {
        self.0.now()
    }
}

/// Sized 桥接：Approval::run 的 A 参数要求具体类型。
struct ApproverShim<'a>(&'a mut (dyn Approver + Send));

impl Approver for ApproverShim<'_> {
    fn poll(&mut self) -> Option<ApprovalVerdict> {
        self.0.poll()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_approver_fifo() {
        let a = QueueApprover::new();
        a.push(ApprovalVerdict::Approved);
        a.push(ApprovalVerdict::Denied);
        let mut boxed: Box<dyn Approver + Send> = Box::new(a.clone());
        assert_eq!(boxed.poll(), Some(ApprovalVerdict::Approved));
        assert_eq!(boxed.poll(), Some(ApprovalVerdict::Denied));
        assert_eq!(boxed.poll(), None);
    }

    #[test]
    fn scripted_clock_advances() {
        let c = ScriptedClock::new(std::time::Duration::ZERO);
        c.advance(std::time::Duration::from_secs(61));
        assert!(c.now() >= std::time::Duration::from_secs(60));
    }

    #[tokio::test]
    async fn line_approver_reads_verdicts() {
        use tokio::io::{AsyncWriteExt, BufReader};
        let (client, server) = tokio::io::duplex(64);
        let mut writer = client;
        let approver = spawn_line_approver(BufReader::new(server)).await;
        writer.write_all(b"approve\ndenY\njunk\n").await.unwrap();
        drop(writer);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let mut boxed: Box<dyn Approver + Send> = Box::new(approver);
        assert_eq!(boxed.poll(), Some(ApprovalVerdict::Approved));
        assert_eq!(boxed.poll(), Some(ApprovalVerdict::Denied));
        assert_eq!(boxed.poll(), None);
    }

    #[tokio::test]
    async fn drive_resolves_scripted_verdict() {
        let scripted = Arc::new(ScriptedClock::new(std::time::Duration::ZERO));
        let clock: Arc<dyn Clock + Send + Sync> = scripted;
        let approver = QueueApprover::new();
        approver.push(ApprovalVerdict::Approved);
        let approver_arc = Arc::new(Mutex::new(Box::new(approver) as Box<dyn Approver + Send>));
        let verdict = drive_approval(clock, approver_arc).await;
        assert_eq!(verdict, ApprovalVerdict::Approved);
    }

    #[tokio::test]
    async fn drive_timeouts_when_unanswered() {
        let scripted = Arc::new(ScriptedClock::new(std::time::Duration::ZERO));
        let clock: Arc<dyn Clock + Send + Sync> = scripted.clone();
        let approver = QueueApprover::new();
        let approver_arc = Arc::new(Mutex::new(Box::new(approver) as Box<dyn Approver + Send>));
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(30));
            scripted.advance(std::time::Duration::from_secs(61));
        });
        let verdict = drive_approval(clock, approver_arc).await;
        assert_eq!(verdict, ApprovalVerdict::Timeout);
    }
}
