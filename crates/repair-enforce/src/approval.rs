//! 审批状态机：pending -> approved / denied / timeout。
//!
//! 语义（remote-support-plan.md §3.4/§5）：write/danger 调用挂起等人工审批，
//! **60s 超时 = 拒绝**，且超时上限不可配置为放行（[APPROVAL_TIMEOUT] 为编译期
//! 常量，[Approval::open] 不接收任何超时参数）。断线语义（§3.7）：挂起中的审批
//! 遇连接断开视同拒绝——由审批通道实现映射为 [ApprovalVerdict::Denied]。
//!
//! 通道经 [Approver] trait 注入（本批不实现具体 UI/CLI 通道，T26 起接线）；
//! 时钟经 [Clock] trait 注入供测试确定性推进。

use std::time::Duration;

/// 审批超时：60 秒，编译期固定，不可配置为放行。
pub const APPROVAL_TIMEOUT: Duration = Duration::from_secs(60);

/// 审批裁决。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalVerdict {
    Approved,
    Denied,
    Timeout,
}

/// 审批状态机状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalState {
    Pending,
    Approved,
    Denied,
    Timeout,
}

/// 可注入时钟：返回自参考时刻起的相对时长（生产实现可用
/// std::time::Instant::elapsed；测试注入脚本化时钟推进时间）。
pub trait Clock {
    fn now(&self) -> Duration;
}

/// 可注入审批通道：poll 返回裁决则状态机终结；返回 None 表示用户侧仍在等待。
/// 通道断开/取消应返回 [ApprovalVerdict::Denied]（断线视同拒绝）。
pub trait Approver {
    fn poll(&mut self) -> Option<ApprovalVerdict>;
}

/// 一次审批会话。
pub struct Approval {
    deadline: Duration,
    state: ApprovalState,
}

impl Approval {
    /// 以固定 60s 预算开启一次审批（deadline = 当前时刻 + 超时常量）。
    pub fn open<C: Clock>(clock: &C) -> Self {
        let deadline = clock
            .now()
            .checked_add(APPROVAL_TIMEOUT)
            .unwrap_or(Duration::MAX);
        Self {
            deadline,
            state: ApprovalState::Pending,
        }
    }

    /// 当前状态。
    pub fn state(&self) -> ApprovalState {
        self.state
    }

    /// 驱动状态机前进：轮询通道，超时即拒；返回终结裁决。
    /// 已终结的会话再调用直接返回既有裁决。
    pub fn run<C: Clock, A: Approver>(&mut self, clock: &C, approver: &mut A) -> ApprovalVerdict {
        if !matches!(self.state, ApprovalState::Pending) {
            return self.final_verdict();
        }
        loop {
            if let Some(v) = approver.poll() {
                self.state = to_state(v);
                return v;
            }
            if clock.now() >= self.deadline {
                self.state = ApprovalState::Timeout;
                return ApprovalVerdict::Timeout;
            }
        }
    }

    fn final_verdict(&self) -> ApprovalVerdict {
        match self.state {
            ApprovalState::Approved => ApprovalVerdict::Approved,
            ApprovalState::Denied => ApprovalVerdict::Denied,
            ApprovalState::Timeout => ApprovalVerdict::Timeout,
            ApprovalState::Pending => ApprovalVerdict::Timeout,
        }
    }
}

fn to_state(v: ApprovalVerdict) -> ApprovalState {
    match v {
        ApprovalVerdict::Approved => ApprovalState::Approved,
        ApprovalVerdict::Denied => ApprovalState::Denied,
        ApprovalVerdict::Timeout => ApprovalState::Timeout,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct FakeClock(RefCell<Duration>);

    impl FakeClock {
        fn new(now: Duration) -> Self {
            FakeClock(RefCell::new(now))
        }
        fn advance(&self, d: Duration) {
            *self.0.borrow_mut() += d;
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Duration {
            *self.0.borrow()
        }
    }

    struct ScriptedApprover(Vec<Option<ApprovalVerdict>>);

    impl Approver for ScriptedApprover {
        fn poll(&mut self) -> Option<ApprovalVerdict> {
            if self.0.is_empty() {
                None
            } else {
                self.0.remove(0)
            }
        }
    }

    #[test]
    fn approval_passes_through() {
        let clock = FakeClock::new(Duration::ZERO);
        let mut approval = Approval::open(&clock);
        assert_eq!(approval.state(), ApprovalState::Pending);
        let mut approver = ScriptedApprover(vec![Some(ApprovalVerdict::Approved)]);
        assert_eq!(
            approval.run(&clock, &mut approver),
            ApprovalVerdict::Approved
        );
        assert_eq!(approval.state(), ApprovalState::Approved);
    }

    #[test]
    fn denial_short_circuits_timeout() {
        let clock = FakeClock::new(Duration::ZERO);
        let mut approval = Approval::open(&clock);
        let mut approver = ScriptedApprover(vec![Some(ApprovalVerdict::Denied)]);
        assert_eq!(approval.run(&clock, &mut approver), ApprovalVerdict::Denied);
        assert_eq!(approval.state(), ApprovalState::Denied);
    }

    #[test]
    fn sixty_second_timeout_is_denial() {
        let clock = FakeClock::new(Duration::ZERO);
        let mut approval = Approval::open(&clock);
        let mut approver = ScriptedApprover(vec![]);
        clock.advance(APPROVAL_TIMEOUT + Duration::from_millis(1));
        assert_eq!(
            approval.run(&clock, &mut approver),
            ApprovalVerdict::Timeout
        );
        assert_eq!(approval.state(), ApprovalState::Timeout);
    }

    #[test]
    fn decision_before_deadline_wins() {
        let clock = FakeClock::new(Duration::ZERO);
        let mut approval = Approval::open(&clock);
        clock.advance(Duration::from_secs(59));
        let mut approver = ScriptedApprover(vec![Some(ApprovalVerdict::Approved)]);
        assert_eq!(
            approval.run(&clock, &mut approver),
            ApprovalVerdict::Approved
        );
    }

    #[test]
    fn suspended_approval_hits_timeout_at_exact_deadline() {
        let clock = FakeClock::new(Duration::ZERO);
        let mut approval = Approval::open(&clock);
        let mut approver = ScriptedApprover(vec![]);
        clock.advance(APPROVAL_TIMEOUT);
        assert_eq!(
            approval.run(&clock, &mut approver),
            ApprovalVerdict::Timeout
        );
    }

    #[test]
    fn timeout_const_is_fixed_not_configurable() {
        // 超时时长是编译期常量：不存在以参数放行的 API 面
        assert_eq!(APPROVAL_TIMEOUT, Duration::from_secs(60));
    }

    #[test]
    fn rerun_after_final_returns_same_verdict() {
        let clock = FakeClock::new(Duration::ZERO);
        let mut approval = Approval::open(&clock);
        let mut approver = ScriptedApprover(vec![Some(ApprovalVerdict::Approved)]);
        assert_eq!(
            approval.run(&clock, &mut approver),
            ApprovalVerdict::Approved
        );
        let mut silent = ScriptedApprover(vec![]);
        assert_eq!(approval.run(&clock, &mut silent), ApprovalVerdict::Approved);
    }
}
