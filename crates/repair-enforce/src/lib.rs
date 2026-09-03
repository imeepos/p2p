//! repair-enforce：本地执法核心（RS P0b 批次一 T22）。
//!
//! 纯逻辑 crate，无网络、无进程、无文件 IO。职责：风险分级（read/write/
//! danger）、五条无条件红线、scope 门（diag/fix）、审批状态机（60s 超时
//! 拒）、shell 白名单闭集匹配语义。契约见 remote-support-plan.md §3.4。
//!
//! 执法顺序（[Enforcer::evaluate]）：红线（无条件拒，先于一切）→ shell
//! 白名单闭集 → 风险分级 → scope 门 →（fix 下 write/danger）审批状态机。

pub mod approval;
pub mod redline;
pub mod redline_data;
pub mod risk;
pub mod scope;
pub mod util;
pub mod whitelist;
pub mod whitelist_data;

#[cfg(test)]
mod redline_bypass_tests;
#[cfg(test)]
mod redline_tests;

pub use approval::{Approval, ApprovalState, ApprovalVerdict, Approver, Clock, APPROVAL_TIMEOUT};
pub use redline::Redline;
pub use risk::{Risk, RiskRule, ToolCall};
pub use scope::{GateDecision, Scope};
pub use whitelist::{ArgPat, ShellDenyReason, ShellRule, ShellWhitelist};
pub use whitelist_data::{builtin, WhitelistEntry, WHITELIST_TABLE};

/// 执法流水线的一次裁决。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// 放行（read 档，任何 scope）。
    Allow,
    /// 命中红线：无条件拒绝。
    Redline(Redline),
    /// shell 命令在白名单闭集之外：拒绝。
    WhitelistDenied,
    /// diag scope 下 write/danger：直接拒绝。
    ScopeDenied,
    /// fix scope 下 write/danger：进入审批状态机。
    NeedApproval(Risk),
}

/// 本地执法引擎。scope 来自工单票据，白名单数据由配置注入（T24 填充）。
pub struct Enforcer<'a> {
    scope: Scope,
    whitelist: &'a ShellWhitelist,
}

impl<'a> Enforcer<'a> {
    pub fn new(scope: Scope, whitelist: &'a ShellWhitelist) -> Self {
        Self { scope, whitelist }
    }

    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// 对一次工具调用做完整执法裁决。
    pub fn evaluate(&self, call: &ToolCall) -> Verdict {
        if let Some(rl) = redline::check_tool_call(call) {
            return Verdict::Redline(rl);
        }
        if call.tool == "shell_exec" {
            if let Some(reason) = self.whitelist.deny_reason(&call.shell_argv()) {
                tracing::debug!(tool = "shell_exec", reason = ?reason, "shell 白名单闭集拒绝");
                return Verdict::WhitelistDenied;
            }
        }
        let risk = risk::classify(call);
        match scope::gate(self.scope, risk) {
            GateDecision::Allow => Verdict::Allow,
            GateDecision::ScopeDenied => Verdict::ScopeDenied,
            GateDecision::NeedApproval => Verdict::NeedApproval(risk),
        }
    }
}
