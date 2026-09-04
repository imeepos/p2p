use repair_enforce::{
    redline, Approval, ApprovalState, ApprovalVerdict, Approver, ArgPat, Clock, Enforcer, Redline,
    Risk, Scope, ShellDenyReason, ShellRule, ShellWhitelist, ToolCall,
};
use std::time::Duration;
fn call(tool: &str, params: &[(&str, &str)]) -> ToolCall {
    ToolCall {
        tool: tool.to_owned(),
        params: params
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    }
}
fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|p| (*p).to_owned()).collect()
}
fn rule(program: &str, args: Vec<ArgPat>) -> ShellRule {
    ShellRule::new(program, args)
}

#[test]
fn credential_path_component_boundaries_are_explicit() {
    assert!(!redline::is_credentials_path("/home/user"));
    assert!(!redline::is_credentials_path("/home/user2"));
    assert!(redline::is_credentials_path(
        "/home/user/credentials-backup"
    ));
    assert!(redline::is_credentials_path("/HOME/User/.SSH/ID_RSA"));
    assert!(redline::is_credentials_path("relative/credentials/file"));
    assert!(!redline::is_credentials_path("relative/user2/report.txt"));
}

#[test]
fn argv_empty_and_program_only_have_closed_set_results() {
    let mut w = ShellWhitelist::empty();
    w.add(rule("tool", vec![]));
    assert_eq!(w.deny_reason(&[]), Some(ShellDenyReason::UnknownProgram));
    assert_eq!(w.deny_reason(&argv(&["tool"])), None);
    assert_eq!(
        w.deny_reason(&argv(&["other"])),
        Some(ShellDenyReason::UnknownProgram)
    );
}

#[test]
fn argv_space_long_and_extra_values_are_checked_positionally() {
    let mut w = ShellWhitelist::empty();
    w.add(rule(
        "tool",
        vec![ArgPat::exact("--name=Alice Smith"), ArgPat::Any],
    ));
    assert!(w.is_allowed(&argv(&["tool", "--name=Alice Smith"])));
    assert!(w.is_allowed(&argv(&["tool", "--name=Alice Smith", "value with spaces"])));
    assert!(!w.is_allowed(&argv(&["tool", "--name=Alice Smith", "value", "extra"])));
    let long = "x".repeat(16_384);
    assert!(!w.is_allowed(&argv(&["tool", &long])));
}

#[test]
fn argv_compound_features_are_rejected_even_when_program_matches() {
    let mut w = ShellWhitelist::empty();
    w.add(rule("tool", vec![ArgPat::Any]));
    for token in ["`whoami`", "$(whoami)", "a\nb", "a|b", "a>b", "a<b"] {
        assert_eq!(
            w.deny_reason(&argv(&["tool", token])),
            Some(ShellDenyReason::CompoundShell),
            "{token:?}"
        );
    }
}

#[test]
fn dollar_without_command_substitution_is_plain_parameter_text() {
    let mut w = ShellWhitelist::empty();
    w.add(rule("tool", vec![ArgPat::Any]));
    assert_eq!(w.deny_reason(&argv(&["tool", "value$HOME"])), None);
    assert_eq!(w.deny_reason(&argv(&["tool", "literal"])), None);
}

#[test]
fn exact_prefix_and_any_patterns_have_strict_boundaries() {
    let mut w = ShellWhitelist::empty();
    w.add(rule(
        "tool",
        vec![
            ArgPat::exact("--mode"),
            ArgPat::prefix("--path="),
            ArgPat::Any,
        ],
    ));
    assert!(w.is_allowed(&argv(&["tool", "--mode", "--path=/tmp", "one"])));
    assert!(!w.is_allowed(&argv(&["tool", "--modeX", "--path=/tmp", "one"])));
    assert!(!w.is_allowed(&argv(&["tool", "--mode", "path=/tmp", "one"])));
    assert!(w.is_allowed(&argv(&["tool", "--mode", "--path="])));
    assert!(!w.is_allowed(&argv(&["tool", "--mode", "--path=/tmp", "one", "two"])));
}

#[test]
fn unknown_tool_remains_danger_and_needs_fix_approval() {
    let c = call("unknown-boundary-tool", &[]);
    assert_eq!(repair_enforce::risk::classify(&c), Risk::Danger);
    let w = ShellWhitelist::empty();
    assert_eq!(
        Enforcer::new(Scope::Fix, &w).evaluate(&c),
        repair_enforce::Verdict::NeedApproval(Risk::Danger)
    );
}

#[test]
fn both_scopes_apply_complete_tool_risk_matrix() {
    let reads = [
        "sys_snapshot",
        "fs_read",
        "fs_list",
        "fs_search",
        "proc_query",
        "svc_query",
        "net_diag",
        "session_report",
    ];
    let writes = ["fs_write", "fs_edit", "backup_point"];
    for scope in [Scope::Diag, Scope::Fix] {
        let w = ShellWhitelist::empty();
        let e = Enforcer::new(scope, &w);
        for tool in reads {
            assert_eq!(
                e.evaluate(&call(tool, &[])),
                repair_enforce::Verdict::Allow,
                "{scope:?} {tool}"
            );
        }
        for tool in writes {
            let expected = if scope == Scope::Diag {
                repair_enforce::Verdict::ScopeDenied
            } else {
                repair_enforce::Verdict::NeedApproval(Risk::Write)
            };
            assert_eq!(e.evaluate(&call(tool, &[])), expected);
        }
        let expected = if scope == Scope::Diag {
            repair_enforce::Verdict::ScopeDenied
        } else {
            repair_enforce::Verdict::NeedApproval(Risk::Danger)
        };
        assert_eq!(e.evaluate(&call("fs_delete", &[])), expected);
    }
}

use std::cell::Cell;
use std::rc::Rc;

struct AdvancingClock(Rc<Cell<Duration>>);
impl Clock for AdvancingClock {
    fn now(&self) -> Duration {
        self.0.get()
    }
}
struct Decision {
    answer: Option<ApprovalVerdict>,
    clock: Rc<Cell<Duration>>,
}
impl Approver for Decision {
    fn poll(&mut self) -> Option<ApprovalVerdict> {
        if self.answer.is_none() {
            self.clock.set(Duration::from_secs(60));
        }
        self.answer.take()
    }
}

#[test]
fn approval_timeout_is_fixed_and_terminal() {
    let time = Rc::new(Cell::new(Duration::ZERO));
    let clock = AdvancingClock(Rc::clone(&time));
    let mut approval = Approval::open(&clock);
    assert_eq!(approval.state(), ApprovalState::Pending);
    let mut silent = Decision {
        answer: None,
        clock: Rc::clone(&time),
    };
    assert_eq!(approval.run(&clock, &mut silent), ApprovalVerdict::Timeout);
    assert_eq!(approval.state(), ApprovalState::Timeout);
    let mut late = Decision {
        answer: Some(ApprovalVerdict::Approved),
        clock: time,
    };
    assert_eq!(approval.run(&clock, &mut late), ApprovalVerdict::Timeout);
}

#[test]
fn redlines_precede_both_scope_decisions() {
    for scope in [Scope::Diag, Scope::Fix] {
        let w = ShellWhitelist::empty();
        let e = Enforcer::new(scope, &w);
        assert_eq!(
            e.evaluate(&call("fs_read", &[("path", "/home/user/.ssh/id_rsa")])),
            repair_enforce::Verdict::Redline(Redline::Credentials)
        );
    }
}
