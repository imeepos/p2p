//! p2p-authz：统一授权层底座（authz-role-design §12 A1）。
//!
//! 模型（§3）：Permission 闭集 key / Role = 权限集合 / Binding peer 级单值 /
//! Decision Allow | Deny{reason}。判定（§7）为四步瀑布纯函数：无绑定
//! NotBound → 过期 Expired → 角色缺失 BrokenRole → 权限不含 MissingPerm → Allow。
//! 存储（§6）：<data>/authz/roles.json 与 bindings.json，version=1 信封，
//! tmp+rename 原子写；损坏/版本不符显式报错拒读，禁止静默回退空表。
//!
//! 零依赖业务 crate：peer 以 base58 字符串承载；内建四角色为代码内闭集
//! （§5，不可改不可删），roles.json 只落自定义角色，引擎合并两源。
//! 时钟经 [clock::Clock] trait 注入（沿 repair-enforce 先例），判定可机械测试。
//!
//! 范围外（A2/A3）：import 迁移、各面 PEP 接入、GUI/审计均不在本 crate。

pub mod binding;
pub mod clock;
pub mod decision;
pub mod engine;
pub mod errors;
pub mod ops;
pub mod ops_binding;
pub mod permissions;
pub mod role;
pub mod store;

#[cfg(test)]
mod engine_tests;

#[cfg(test)]
mod ops_tests;

#[cfg(test)]
mod store_tests;

pub use binding::Binding;
pub use clock::{Clock, SystemClock};
pub use decision::{Decision, DenyReason};
pub use engine::AuthzEngine;
pub use errors::AuthzError;
pub use ops::Authz;
pub use ops_binding::BoundBinding;
pub use permissions::Permission;
pub use role::{validate_role_id, Role, BUILTIN_ROLE_IDS};
pub use store::{AuthzStoreError, FILE_VERSION};
