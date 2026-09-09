//! 绑定与判定操作（authz-role-design §10）：bind（upsert，--expires Unix 秒）/
//! unbind / check（dry-run 判定，Clock 注入 now）。测试见 [crate::ops_tests]。

use crate::binding::Binding;
use crate::clock::Clock;
use crate::decision::Decision;
use crate::errors::AuthzError;
use crate::ops::Authz;
use crate::permissions::Permission;
use crate::store;

/// bind 的返回：落库后的绑定 + 是否新建（false = upsert 更新）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundBinding {
    pub binding: Binding,
    pub created: bool,
}

impl<C: Clock> Authz<C> {
    /// 绑定 peer → 角色（单值 upsert）；角色必须存在（内建或自定义）。
    pub fn bind(
        &self,
        peer_id: &str,
        role_id: &str,
        expires_at: Option<u64>,
        note: &str,
    ) -> Result<BoundBinding, AuthzError> {
        let mut engine = self.load_engine()?;
        if engine.lookup_role(role_id).is_none() {
            return Err(AuthzError::RoleNotFound(role_id.to_owned()));
        }
        let binding = Binding {
            peer_id: peer_id.to_owned(),
            role_id: role_id.to_owned(),
            granted_at: self.clock.now_unix(),
            note: note.to_owned(),
            expires_at,
        };
        let created = engine.binding(peer_id).is_none();
        engine.upsert_binding(binding.clone());
        store::save_bindings(&self.data_dir, &engine.bindings())?;
        Ok(BoundBinding { binding, created })
    }

    /// 解绑；条目不存在显式报错 NotBound（无绑即默认拒绝，无需 unbind）。
    pub fn unbind(&self, peer_id: &str) -> Result<Binding, AuthzError> {
        let mut engine = self.load_engine()?;
        let binding = engine
            .remove_binding(peer_id)
            .ok_or_else(|| AuthzError::NotBound(peer_id.to_owned()))?;
        store::save_bindings(&self.data_dir, &engine.bindings())?;
        Ok(binding)
    }

    /// dry-run 判定（§7）：只读，Clock 注入当前时刻。
    pub fn check(&self, peer_id: &str, perm: Permission) -> Result<Decision, AuthzError> {
        let engine = self.load_engine()?;
        Ok(engine.check(peer_id, perm, self.clock.now_unix()))
    }
}
