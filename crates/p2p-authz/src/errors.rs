//! 错误类型：存储层错误（[AuthzStoreError]）与领域操作错误（[AuthzError]）。
//! 失败路径一律显式错误上抛，不做静默回退（§11 红线 2）。

use crate::store::AuthzStoreError;

/// 领域操作错误：管理面命令与绑定操作的失败语义。
#[derive(Debug, thiserror::Error)]
pub enum AuthzError {
    /// 底层存储不可读/损坏/版本不符（透明透出 reason 链）。
    #[error(transparent)]
    Store(#[from] AuthzStoreError),
    #[error("角色不存在: {0}")]
    RoleNotFound(String),
    #[error("角色已存在: {0}")]
    RoleExists(String),
    #[error("内建角色不可创建、修改或删除: {0}")]
    BuiltinImmutable(String),
    #[error("角色仍有绑定引用，先解绑再删除: {0}")]
    RoleReferenced(String),
    #[error("权限 key 不在闭集登记表: {0}")]
    UnknownPermission(String),
    #[error("{0}")]
    InvalidRoleId(String),
    #[error("该 peer 无绑定: {0}")]
    NotBound(String),
}
