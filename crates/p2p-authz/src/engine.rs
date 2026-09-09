//! 判定引擎（authz-role-design §7 四步瀑布，纯内存无 IO）：
//! 绑定缺失 NotBound → 过期 Expired → 角色缺失 BrokenRole → 权限不含 MissingPerm → Allow。
//! 角色两源合并：内建四角色（代码内闭集）优先，自定义角色来自 roles.json；
//! 自定义角色占用内建 id 属防御性丢弃（告警可观测，正常路径被 ops 层前置拒绝）。
//! 瀑布分支测试见 [crate::engine_tests]。

use std::collections::BTreeMap;

use tracing::warn;

use crate::binding::Binding;
use crate::decision::{Decision, DenyReason};
use crate::errors::AuthzError;
use crate::permissions::Permission;
use crate::role::{builtin_role, builtin_roles, Role};

/// 内存判定引擎：custom_roles / bindings 均按主键 BTreeMap 有序。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthzEngine {
    custom_roles: BTreeMap<String, Role>,
    bindings: BTreeMap<String, Binding>,
}

impl AuthzEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// 由存储层两表装配；与内建同 id 的自定义角色防御性丢弃并告警。
    pub fn from_parts(custom_roles: Vec<Role>, bindings: Vec<Binding>) -> Self {
        let mut engine = Self::new();
        for role in custom_roles {
            if builtin_role(&role.role_id).is_some() {
                warn!(role_id = %role.role_id, "自定义角色与内建角色同 id，防御性丢弃");
                continue;
            }
            engine.custom_roles.insert(role.role_id.clone(), role);
        }
        for binding in bindings {
            engine.bindings.insert(binding.peer_id.clone(), binding);
        }
        engine
    }

    /// §7 判定瀑布（纯函数；now_unix 由调用方注入）。
    pub fn check(&self, peer_id: &str, perm: Permission, now_unix: u64) -> Decision {
        let Some(binding) = self.bindings.get(peer_id) else {
            return Decision::Deny(DenyReason::NotBound);
        };
        if binding.expired_at(now_unix) {
            return Decision::Deny(DenyReason::Expired);
        }
        let Some(role) = self.lookup_role(&binding.role_id) else {
            return Decision::Deny(DenyReason::BrokenRole);
        };
        if !role.has(perm) {
            return Decision::Deny(DenyReason::MissingPerm);
        }
        Decision::Allow
    }

    /// 角色查找：内建优先（不可改语义：同 id 自定义即便溜入也不生效）。
    pub fn lookup_role(&self, role_id: &str) -> Option<Role> {
        builtin_role(role_id).or_else(|| self.custom_roles.get(role_id).cloned())
    }

    /// 全量角色视图：内建固定序在前，自定义按 id 有序随后。
    pub fn roles(&self) -> Vec<Role> {
        let mut all = builtin_roles();
        all.extend(self.custom_roles.values().cloned());
        all
    }

    pub fn custom_roles(&self) -> Vec<Role> {
        self.custom_roles.values().cloned().collect()
    }

    pub fn binding(&self, peer_id: &str) -> Option<&Binding> {
        self.bindings.get(peer_id)
    }

    pub fn bindings(&self) -> Vec<Binding> {
        self.bindings.values().cloned().collect()
    }

    /// 角色的绑定引用数（删除前引用完整性检查用）。
    pub fn role_binding_count(&self, role_id: &str) -> usize {
        self.bindings
            .values()
            .filter(|b| b.role_id == role_id)
            .count()
    }

    /// 新增自定义角色：内建同 id / 已存在均显式拒绝。
    pub fn insert_custom_role(&mut self, role: Role) -> Result<(), AuthzError> {
        if role.builtin || builtin_role(&role.role_id).is_some() {
            return Err(AuthzError::BuiltinImmutable(role.role_id));
        }
        if self.custom_roles.contains_key(&role.role_id) {
            return Err(AuthzError::RoleExists(role.role_id));
        }
        self.custom_roles.insert(role.role_id.clone(), role);
        Ok(())
    }

    /// 删除自定义角色：内建不可删；不存在显式报错（引用完整性由 ops 层前置检查）。
    pub fn remove_custom_role(&mut self, role_id: &str) -> Result<Role, AuthzError> {
        if builtin_role(role_id).is_some() {
            return Err(AuthzError::BuiltinImmutable(role_id.to_owned()));
        }
        self.custom_roles
            .remove(role_id)
            .ok_or_else(|| AuthzError::RoleNotFound(role_id.to_owned()))
    }

    /// upsert 绑定（单值语义）；返回是否新建条目。
    pub fn upsert_binding(&mut self, binding: Binding) -> bool {
        self.bindings
            .insert(binding.peer_id.clone(), binding)
            .is_none()
    }

    /// 解绑；条目不存在返回 None（由调用方转 NotBound）。
    pub fn remove_binding(&mut self, peer_id: &str) -> Option<Binding> {
        self.bindings.remove(peer_id)
    }
}
