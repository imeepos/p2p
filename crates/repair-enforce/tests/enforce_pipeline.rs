//! 执法流水线集成测试：红线优先、白名单闭集、scope 门、审批状态机全链路。
//! 仅使用公开 API（crate 外部视角）。

use repair_enforce::approval::{Approval, ApprovalVerdict, Approver, Clock};
use repair_enforce::{
    ApprovalState, ArgPat, Enforcer, Redline, Risk, Scope, ShellRule, ShellWhitelist, ToolCall,
};
use std::cell::RefCell;
use std::time::Duration;

fn call(tool: &str, params: &[(&str, &str)]) -> ToolCall {
    ToolCall {
        tool: tool.to_string(),
        params: params
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

fn empty_whitelist() -> ShellWhitelist {
    ShellWhitelist::empty()
}

/// 白名单内含 whoami 与 tasklist 的测试闭集。
fn diag_whitelist() -> ShellWhitelist {
    let mut w = ShellWhitelist::empty();
    w.add(ShellRule::new("whoami", vec![]));
    w.add(ShellRule::new("tasklist", vec![ArgPat::Any]));
    w
}

#[test]
fn redline_beats_scope_and_whitelist() {
    // diag 下红线上面的工具调用：红线优先，不落到 scope/白名单
    let w = empty_whitelist();
    let e = Enforcer::new(Scope::Diag, &w);
    assert_eq!(
        e.evaluate(&call("shell_exec", &[("argv", "format c:")])),
        verdict_ref::redline(Redline::FormatDisk)
    );
    assert_eq!(
        e.evaluate(&call("fs_read", &[("path", "C:/Users/Jane/.ssh/id_rsa")])),
        verdict_ref::redline(Redline::Credentials)
    );
}

#[test]
fn redline_beats_approval_in_fix() {
    let w = empty_whitelist();
    let e = Enforcer::new(Scope::Fix, &w);
    assert_eq!(
        e.evaluate(&call("shell_exec", &[("argv", "rm -rf /home/jane")])),
        verdict_ref::redline(Redline::BatchDelete)
    );
}

#[test]
fn whitelist_closed_set_rejects_shell() {
    // 空闭集 + diag：任何 shell 先吃白名单拒绝（闭集外一律拒）
    let w = empty_whitelist();
    let e = Enforcer::new(Scope::Diag, &w);
    assert_eq!(
        e.evaluate(&call("shell_exec", &[("argv", "whoami")])),
        verdict_ref::whitelist_denied()
    );
}

#[test]
fn whitelisted_shell_in_diag_hits_scope_gate() {
    let w = diag_whitelist();
    let e = Enforcer::new(Scope::Diag, &w);
    // 白名单放行（read-ish 命令）→ 风险档 Write → diag 直接拒
    assert_eq!(
        e.evaluate(&call("shell_exec", &[("argv", "whoami")])),
        verdict_ref::scope_denied()
    );
}

#[test]
fn read_passes_in_both_scopes() {
    for scope in [Scope::Diag, Scope::Fix] {
        let w = empty_whitelist();
        let e = Enforcer::new(scope, &w);
        assert_eq!(
            e.evaluate(&call(
                "fs_read",
                &[("path", "C:/Users/Jane/Documents/report.txt")]
            )),
            verdict_ref::allow()
        );
        assert_eq!(e.evaluate(&call("sys_snapshot", &[])), verdict_ref::allow());
    }
}

#[test]
fn diag_denies_write_danger() {
    let w = empty_whitelist();
    let e = Enforcer::new(Scope::Diag, &w);
    assert_eq!(
        e.evaluate(&call("fs_write", &[("path", "C:/a.txt")])),
        verdict_ref::scope_denied()
    );
}

#[test]
fn fix_requests_approval_for_write() {
    let w = empty_whitelist();
    let e = Enforcer::new(Scope::Fix, &w);
    assert_eq!(
        e.evaluate(&call("fs_write", &[("path", "C:/a.txt")])),
        verdict_ref::need_approval(Risk::Write)
    );
}

#[test]
fn shell_sensitive_command_is_danger_and_needs_approval_in_fix() {
    let mut w = ShellWhitelist::empty();
    w.add(ShellRule::new("del", vec![ArgPat::Any]));
    let e = Enforcer::new(Scope::Fix, &w);
    assert_eq!(
        e.evaluate(&call("shell_exec", &[("argv", "del C:\\temp\\a.txt")])),
        verdict_ref::need_approval(Risk::Danger)
    );
}

#[test]
fn unknown_tool_is_danger() {
    let w = empty_whitelist();
    let e = Enforcer::new(Scope::Fix, &w);
    assert_eq!(
        e.evaluate(&call("mystery_tool", &[])),
        verdict_ref::need_approval(Risk::Danger)
    );
}

#[test]
fn approval_state_machine_round_trip() {
    let mut w = ShellWhitelist::empty();
    w.add(ShellRule::new("whoami", vec![]));
    let e = Enforcer::new(Scope::Fix, &w);
    assert_eq!(
        e.evaluate(&call("fs_write", &[("path", "C:/a.txt")])),
        verdict_ref::need_approval(Risk::Write)
    );

    struct InstantClock(u64);
    impl Clock for InstantClock {
        fn now(&self) -> Duration {
            Duration::from_secs(self.0)
        }
    }
    struct YesApprover;
    impl Approver for YesApprover {
        fn poll(&mut self) -> Option<ApprovalVerdict> {
            Some(ApprovalVerdict::Approved)
        }
    }

    let clock = InstantClock(0);
    let mut approval = Approval::open(&clock);
    assert_eq!(approval.state(), ApprovalState::Pending);
    assert_eq!(
        approval.run(&clock, &mut YesApprover),
        ApprovalVerdict::Approved
    );
    assert_eq!(approval.state(), ApprovalState::Approved);
}

#[test]
fn bridge_drop_is_modeled_as_denied() {
    // §3.7：挂起审批遇断线 = 拒绝。通道侧把断线映射为 Denied 即可。
    struct DroppedApprover;
    impl Approver for DroppedApprover {
        fn poll(&mut self) -> Option<ApprovalVerdict> {
            Some(ApprovalVerdict::Denied)
        }
    }
    struct FixedClock(RefCell<Duration>);
    impl Clock for FixedClock {
        fn now(&self) -> Duration {
            *self.0.borrow()
        }
    }
    let clock = FixedClock(RefCell::new(Duration::ZERO));
    let mut approval = Approval::open(&clock);
    assert_eq!(
        approval.run(&clock, &mut DroppedApprover),
        ApprovalVerdict::Denied
    );
}

/// 裁决断言的扁平化助手（避免对每个变体写 match）。
mod verdict_ref {
    use repair_enforce::{Redline, Risk, Verdict};

    pub fn redline(r: Redline) -> Verdict {
        Verdict::Redline(r)
    }
    pub fn whitelist_denied() -> Verdict {
        Verdict::WhitelistDenied
    }
    pub fn scope_denied() -> Verdict {
        Verdict::ScopeDenied
    }
    pub fn need_approval(r: Risk) -> Verdict {
        Verdict::NeedApproval(r)
    }
    pub fn allow() -> Verdict {
        Verdict::Allow
    }
}
