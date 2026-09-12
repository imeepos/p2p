//! 管理面操作（authz-role-design §10 命令的库面）：角色 list/show/create/update/delete。
//! 每次操作「重读磁盘 → 内存变更 → 原子写回」（§6：缩小 GUI/CLI 双进程覆盖窗口）；
//! Clock 注入供判定时刻与 granted_at。测试见 [crate::ops_tests]。

use std::path::{Path, PathBuf};

use crate::clock::Clock;
use crate::engine::AuthzEngine;
use crate::errors::AuthzError;
use crate::permissions::Permission;
use crate::role::{validate_role_id, Role};
use crate::store;

/// authz 管理面：data_dir 指向节点数据根（authz 表在其 authz/ 子目录下）。
pub struct Authz<C: Clock> {
    pub(crate) data_dir: PathBuf,
    pub(crate) clock: C,
}

impl<C: Clock> Authz<C> {
    pub fn new(data_dir: &Path, clock: C) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            clock,
        }
    }

    pub fn clock(&self) -> &C {
        &self.clock
    }

    /// authz 数据根目录（诊断与测试读取落盘态用）。
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// 全量角色：内建在前（固定序）+ 自定义按 id 有序。
    pub fn list_roles(&self) -> Result<Vec<Role>, AuthzError> {
        let engine = self.load_engine()?;
        Ok(engine.roles())
    }

    /// 角色详情：内建与自定义同查。
    pub fn show_role(&self, role_id: &str) -> Result<Role, AuthzError> {
        self.load_engine()?
            .lookup_role(role_id)
            .ok_or_else(|| AuthzError::RoleNotFound(role_id.to_owned()))
    }

    /// 创建自定义角色：id 校验 → 权限闭集解析去重 → 引擎插入（内建/重复显式拒）→ 落盘。
    pub fn create_role(
        &self,
        role_id: &str,
        name: &str,
        perm_keys: &[String],
        note: &str,
    ) -> Result<Role, AuthzError> {
        validate_role_id(role_id).map_err(|_| AuthzError::InvalidRoleId(role_id.to_owned()))?;
        let role = Role {
            role_id: role_id.to_owned(),
            name: name.to_owned(),
            permissions: parse_perms(perm_keys)?,
            builtin: false,
            note: note.to_owned(),
        };
        let mut engine = self.load_engine()?;
        engine.insert_custom_role(role.clone())?;
        store::save_roles(&self.data_dir, &engine.custom_roles())?;
        Ok(role)
    }

    /// 修改自定义角色：仅自定义可改（内建显式拒 BuiltinImmutable；未登记
    /// RoleNotFound）；role_id 不可变（改 id = 删+建），name/permissions/note
    /// 整体替换；权限闭集解析去重；成功后 save_roles(custom_roles) 落盘。
    pub fn update_role(
        &self,
        role_id: &str,
        name: &str,
        perm_keys: &[String],
        note: &str,
    ) -> Result<Role, AuthzError> {
        let mut engine = self.load_engine()?;
        match engine.lookup_role(role_id) {
            // 内建与自定义的失败语义显式分叉（§10：内建不可改不可删）。
            Some(role) if role.builtin => {
                return Err(AuthzError::BuiltinImmutable(role_id.to_owned()));
            }
            Some(_) => {}
            None => return Err(AuthzError::RoleNotFound(role_id.to_owned())),
        }
        let role = Role {
            role_id: role_id.to_owned(),
            name: name.to_owned(),
            permissions: parse_perms(perm_keys)?,
            builtin: false,
            note: note.to_owned(),
        };
        // 同 id 先删后插 = 字段级替换；引擎为本操作独享副本，失败不落盘无半态。
        engine.remove_custom_role(role_id)?;
        engine.insert_custom_role(role.clone())?;
        store::save_roles(&self.data_dir, &engine.custom_roles())?;
        Ok(role)
    }

    /// 删除自定义角色：引用完整性前置（有绑定即拒，先解绑再删，§6）。
    pub fn delete_role(&self, role_id: &str) -> Result<Role, AuthzError> {
        let mut engine = self.load_engine()?;
        if engine.role_binding_count(role_id) > 0 {
            return Err(AuthzError::RoleReferenced(role_id.to_owned()));
        }
        let role = engine.remove_custom_role(role_id)?;
        store::save_roles(&self.data_dir, &engine.custom_roles())?;
        Ok(role)
    }

    pub(crate) fn load_engine(&self) -> Result<AuthzEngine, AuthzError> {
        let roles = store::load_roles(&self.data_dir)?;
        let bindings = store::load_bindings(&self.data_dir)?;
        Ok(AuthzEngine::from_parts(roles, bindings))
    }
}

/// 闭集解析 + 保序去重；表外 key 显式报错（UnknownPermission）。
fn parse_perms(perm_keys: &[String]) -> Result<Vec<Permission>, AuthzError> {
    let mut perms = Vec::with_capacity(perm_keys.len());
    for key in perm_keys {
        let perm =
            Permission::parse(key).ok_or_else(|| AuthzError::UnknownPermission(key.clone()))?;
        if !perms.contains(&perm) {
            perms.push(perm);
        }
    }
    Ok(perms)
}
