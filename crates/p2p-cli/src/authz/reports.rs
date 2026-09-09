//! 管理面报告类型（camelCase，--json 输出的事实源）：视图由 p2p-authz
//! 领域对象映射，决策码与 reason 直接采用 §3/§7 命名（NotBound 等）。

use serde::Serialize;

use p2p_authz::{BoundBinding, Decision, Permission, Role};

/// 角色视图：permissions 为 §4 闭集 key 字符串。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleView {
    pub role_id: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub builtin: bool,
    pub note: String,
}

impl From<&Role> for RoleView {
    fn from(role: &Role) -> Self {
        Self {
            role_id: role.role_id.clone(),
            name: role.name.clone(),
            permissions: role
                .permissions
                .iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
            builtin: role.builtin,
            note: role.note.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleListReport {
    pub roles: Vec<RoleView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleShowReport {
    pub role: RoleView,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleCreateReport {
    pub role: RoleView,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleDeleteReport {
    pub role_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindReport {
    pub peer_id: String,
    pub role_id: String,
    /// false = 条目已存在，本次为 upsert 更新。
    pub created: bool,
    pub granted_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    pub note: String,
}

impl From<BoundBinding> for BindReport {
    fn from(bound: BoundBinding) -> Self {
        Self {
            peer_id: bound.binding.peer_id,
            role_id: bound.binding.role_id,
            created: bound.created,
            granted_at: bound.binding.granted_at,
            expires_at: bound.binding.expires_at,
            note: bound.binding.note,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnbindReport {
    pub peer_id: String,
    pub role_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckReport {
    pub peer_id: String,
    pub permission: String,
    /// "allow" | "deny"。
    pub decision: &'static str,
    /// deny 时的 reason 码（NotBound/Expired/BrokenRole/MissingPerm）。
    pub reason: Option<&'static str>,
}

impl CheckReport {
    pub(crate) fn of(peer_id: &str, perm: Permission, decision: Decision) -> Self {
        let (code, reason) = match decision {
            Decision::Allow => ("allow", None),
            Decision::Deny(reason) => ("deny", Some(reason.code())),
        };
        Self {
            peer_id: peer_id.to_owned(),
            permission: perm.as_str().to_owned(),
            decision: code,
            reason,
        }
    }
}
