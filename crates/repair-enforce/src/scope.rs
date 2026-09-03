//! scope 门：diag 下 write/danger 一律直接拒；fix 下进入审批状态机。
//! 语义：remote-support-plan.md §3.4——read 档任何 scope 放行，
//! write/danger 在 diag 直接拒，在 fix 需要人工审批。

use crate::risk::Risk;

/// 工单 scope。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// 只读诊断：write/danger 直接拒。
    Diag,
    /// 修复：write/danger 进入审批。
    Fix,
}

/// scope 门的裁决。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateDecision {
    /// 放行（read，或 diag 下 read）。
    Allow,
    /// diag 下 write/danger，直接拒。
    ScopeDenied,
    /// fix 下 write/danger，进入审批。
    NeedApproval,
}

/// scope 门判定：按风险档与 scope 组合给出裁决，不依赖工具名。
pub fn gate(scope: Scope, risk: Risk) -> GateDecision {
    match risk {
        Risk::Read => GateDecision::Allow,
        Risk::Write | Risk::Danger => match scope {
            Scope::Diag => GateDecision::ScopeDenied,
            Scope::Fix => GateDecision::NeedApproval,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_allowed_everywhere() {
        assert_eq!(gate(Scope::Diag, Risk::Read), GateDecision::Allow);
        assert_eq!(gate(Scope::Fix, Risk::Read), GateDecision::Allow);
    }

    #[test]
    fn diag_denies_write_and_danger() {
        assert_eq!(gate(Scope::Diag, Risk::Write), GateDecision::ScopeDenied);
        assert_eq!(gate(Scope::Diag, Risk::Danger), GateDecision::ScopeDenied);
    }

    #[test]
    fn fix_routes_to_approval() {
        assert_eq!(gate(Scope::Fix, Risk::Write), GateDecision::NeedApproval);
        assert_eq!(gate(Scope::Fix, Risk::Danger), GateDecision::NeedApproval);
    }
}
