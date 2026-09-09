//! 判定结果（authz-role-design §3/§7）：Allow 或 Deny 带 reason 码。
//! Deny reason 入审计只报码，不泄细节（§12-Q5 同口径）。

use std::fmt;

/// 一次判定的裁决。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny(DenyReason),
}

/// 拒绝原因码（§7 瀑布顺序：NotBound → Expired → BrokenRole → MissingPerm）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyReason {
    /// 绑定表无该 peer（默认拒绝）。
    NotBound,
    /// 绑定已过期。
    Expired,
    /// 绑定引用的角色不存在（防御性，正常路径不该出现）。
    BrokenRole,
    /// 角色权限集不含该权限。
    MissingPerm,
}

impl DenyReason {
    /// reason 码字面量（与设计 §3 命名逐字一致，审计面直接可用）。
    pub fn code(self) -> &'static str {
        match self {
            DenyReason::NotBound => "NotBound",
            DenyReason::Expired => "Expired",
            DenyReason::BrokenRole => "BrokenRole",
            DenyReason::MissingPerm => "MissingPerm",
        }
    }
}

impl fmt::Display for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Decision::Allow => f.write_str("Allow"),
            Decision::Deny(reason) => write!(f, "Deny({})", reason.code()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_reason_codes_match_design_naming() {
        assert_eq!(DenyReason::NotBound.code(), "NotBound");
        assert_eq!(DenyReason::Expired.code(), "Expired");
        assert_eq!(DenyReason::BrokenRole.code(), "BrokenRole");
        assert_eq!(DenyReason::MissingPerm.code(), "MissingPerm");
    }

    #[test]
    fn decision_display_is_audit_ready() {
        assert_eq!(Decision::Allow.to_string(), "Allow");
        assert_eq!(
            Decision::Deny(DenyReason::Expired).to_string(),
            "Deny(Expired)"
        );
    }
}
