//! authz 管理面逻辑层（authz-role-design §10/§12 A1）：角色与绑定命令的
//! 纯逻辑面 + 报告构造，clap 命令面在 apps/cli/src/authz/（沿 llm_share 分工先例）。
//! 判定与存储全部委托 p2p-authz（四步瀑布/version 信封/原子写/损坏显式报错），
//! 本层只做参数规整、peer 校验与 Report 形状（camelCase，供 --json 输出）。

pub mod access;
pub mod reports;
pub mod roles;

use std::path::Path;

use p2p_authz::{Authz, AuthzError, SystemClock};

pub use access::{bind, check, unbind};
pub use reports::{
    BindReport, CheckReport, RoleCreateReport, RoleDeleteReport, RoleListReport, RoleShowReport,
    RoleView, UnbindReport,
};
pub use roles::{role_create, role_delete, role_list, role_show};

/// 管理面入口：数据根 + 系统时钟（判定时刻由 p2p-authz 内部注入）。
pub(crate) fn facade(data_dir: &str) -> Authz<SystemClock> {
    Authz::new(Path::new(data_dir), SystemClock)
}

/// peer 校验复用 llm_share 同一语义（base58 解码恰 32 字节），不另造轮子。
pub(crate) fn validate_peer(peer_id: &str) -> Result<(), String> {
    crate::llm_share::validate_peer_id(peer_id)
}

/// p2p-authz 类型化错误 → 逻辑层 String 错误（apps/cli 转 Runtime 退出码 1）。
pub(crate) fn domain_err(e: AuthzError) -> String {
    e.to_string()
}

#[cfg(test)]
mod tests;
