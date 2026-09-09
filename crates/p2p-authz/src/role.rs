//! 角色模型与内建角色（authz-role-design §5）：内建四个呈阶梯
//! （friend ⊂ guest ⊂ operator ⊂ ally），代码内闭集不可改不可删；
//! 自定义角色 = §4 闭集任意子集，role_id 命名 [a-z0-9-]{1,32}，与内建名冲突即拒。

use serde::{Deserialize, Serialize};

use crate::permissions::Permission;

/// 自定义角色 id 长度上限（§5）。
pub const ROLE_ID_MAX_LEN: usize = 32;

/// 内建角色 id 闭集（判定层防御：自定义角色不得占用）。
pub const BUILTIN_ROLE_IDS: [&str; 4] = ["friend", "guest", "operator", "ally"];

/// 角色 = 权限集合；builtin=true 的实例只来自 [builtin_roles]。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    pub role_id: String,
    pub name: String,
    pub permissions: Vec<Permission>,
    pub builtin: bool,
    #[serde(default)]
    pub note: String,
}

impl Role {
    pub fn has(&self, perm: Permission) -> bool {
        self.permissions.contains(&perm)
    }
}

/// 自定义角色 id 校验：非空、≤32 字节、字符仅 a-z0-9-（§5 正则的免 regex 落地）。
pub fn validate_role_id(role_id: &str) -> Result<(), String> {
    let valid = role_id.len() <= ROLE_ID_MAX_LEN
        && !role_id.is_empty()
        && role_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if valid {
        Ok(())
    } else {
        Err(format!("角色 id 非法（应为 [a-z0-9-]{{1,32}}）：{role_id}"))
    }
}

/// 内建角色表（§5 语义注记随之）：阶梯定义只此一处。
pub fn builtin_roles() -> Vec<Role> {
    vec![
        Role {
            role_id: "friend".to_owned(),
            name: "好友".to_owned(),
            permissions: vec![
                Permission::CHAT_SEND,
                Permission::CHAT_ATTACHMENT,
                Permission::A2A_DISCOVER,
            ],
            builtin: true,
            note: "基础社交；加好友默认绑定（default_role，可配）".to_owned(),
        },
        Role {
            role_id: "guest".to_owned(),
            name: "访客".to_owned(),
            permissions: vec![
                Permission::CHAT_SEND,
                Permission::CHAT_ATTACHMENT,
                Permission::A2A_DISCOVER,
                Permission::ACP_SESSION,
            ],
            builtin: true,
            note: "能看能问 agent（执行细节仍在策略表，sandbox 语义）".to_owned(),
        },
        Role {
            role_id: "operator".to_owned(),
            name: "操作员".to_owned(),
            permissions: vec![
                Permission::CHAT_SEND,
                Permission::CHAT_ATTACHMENT,
                Permission::A2A_DISCOVER,
                Permission::ACP_SESSION,
                Permission::A2A_INVOKE,
                Permission::ACP_EXECUTE,
                Permission::REPAIR_DIAG,
            ],
            builtin: true,
            note: "能操作 agent、跑诊断".to_owned(),
        },
        Role {
            role_id: "ally".to_owned(),
            name: "盟友".to_owned(),
            permissions: vec![
                Permission::CHAT_SEND,
                Permission::CHAT_ATTACHMENT,
                Permission::A2A_DISCOVER,
                Permission::ACP_SESSION,
                Permission::A2A_INVOKE,
                Permission::ACP_EXECUTE,
                Permission::REPAIR_DIAG,
                Permission::LLM_BORROW,
                Permission::REPAIR_FIX,
            ],
            builtin: true,
            note: "全面信任；可借额度、可修机器".to_owned(),
        },
    ]
}

/// 按 id 查内建角色。
pub fn builtin_role(role_id: &str) -> Option<Role> {
    builtin_roles().into_iter().find(|r| r.role_id == role_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §5 逐字锚点：改内建表即测试红。
    const FRIEND: [&str; 3] = ["chat.send", "chat.attachment", "a2a.discover"];
    const GUEST_EXTRA: [&str; 1] = ["acp.session"];
    const OPERATOR_EXTRA: [&str; 3] = ["a2a.invoke", "acp.execute", "repair.diag"];
    const ALLY_EXTRA: [&str; 2] = ["llm.borrow", "repair.fix"];

    fn key_set(role: &Role) -> Vec<&str> {
        let mut keys: Vec<&str> = role.permissions.iter().map(|p| p.as_str()).collect();
        keys.sort_unstable();
        keys
    }

    fn sorted<'a>(keys: &[&'a str]) -> Vec<&'a str> {
        let mut own: Vec<&'a str> = keys.to_vec();
        own.sort_unstable();
        own
    }

    fn role_of(id: &str) -> Role {
        builtin_role(id).unwrap_or_else(|| panic!("内建角色缺失: {id}"))
    }

    #[test]
    fn builtin_ladder_matches_design_verbatim() {
        let guest_all: Vec<&str> = FRIEND.iter().chain(GUEST_EXTRA.iter()).copied().collect();
        let operator_all: Vec<&str> = guest_all
            .iter()
            .chain(OPERATOR_EXTRA.iter())
            .copied()
            .collect();
        let ally_all: Vec<&str> = operator_all
            .iter()
            .chain(ALLY_EXTRA.iter())
            .copied()
            .collect();
        assert_eq!(key_set(&role_of("friend")), sorted(&FRIEND));
        assert_eq!(key_set(&role_of("guest")), sorted(&guest_all));
        assert_eq!(key_set(&role_of("operator")), sorted(&operator_all));
        assert_eq!(key_set(&role_of("ally")), sorted(&ally_all));
    }

    #[test]
    fn builtin_ladder_is_strict_inclusion() {
        let ladder: Vec<std::collections::BTreeSet<Permission>> = BUILTIN_ROLE_IDS
            .iter()
            .map(|id| role_of(id).permissions.into_iter().collect())
            .collect();
        for pair in ladder.windows(2) {
            assert!(
                pair[0].is_subset(&pair[1]) && pair[0] != pair[1],
                "阶梯必须严格递增"
            );
        }
    }

    #[test]
    fn builtin_ids_flags_and_registry_closure() {
        assert_eq!(BUILTIN_ROLE_IDS, ["friend", "guest", "operator", "ally"]);
        let roles = builtin_roles();
        assert_eq!(roles.len(), 4);
        for role in roles {
            assert!(role.builtin);
            for perm in &role.permissions {
                assert!(
                    Permission::registry().contains(perm),
                    "内建角色权限 {} 不在 §4 登记表",
                    perm
                );
            }
        }
    }

    #[test]
    fn role_id_validation_bounds() {
        assert!(validate_role_id("friend").is_ok());
        assert!(validate_role_id("a-1").is_ok());
        assert!(validate_role_id(&"a".repeat(32)).is_ok());
        assert!(validate_role_id("").is_err());
        assert!(validate_role_id("Friend").is_err());
        assert!(validate_role_id("a_b").is_err());
        assert!(validate_role_id("角色").is_err());
        assert!(validate_role_id(&"a".repeat(33)).is_err());
    }

    #[test]
    fn role_has_checks_membership() {
        let ally = role_of("ally");
        assert!(ally.has(Permission::REPAIR_FIX));
        assert!(!builtin_role("friend").unwrap().has(Permission::REPAIR_FIX));
    }
}
