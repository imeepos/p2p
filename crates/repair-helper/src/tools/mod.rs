//! 只读工具面装配：sys_snapshot/fs_read/fs_list/fs_search 四工具 + 注册入口。
//!
//! 四个工具全部为 read 档（风险档判定在 repair-enforce RISK_RULES，本批恒
//! 判 read，见 remote-support-plan.md §3.5 工具表）。

pub mod fs_list;
pub mod fs_read;
pub mod fs_search;
pub mod glob;
pub mod sys_snapshot;

use crate::jail::PathJail;
use crate::ToolRegistry;

/// 装配四个只读工具到新注册表，返回宿主可直接使用的注册表。
pub fn read_only_registry(jail: PathJail) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(sys_snapshot::SysSnapshot::new());
    registry.register(fs_read::FsRead::new(jail.clone()));
    registry.register(fs_list::FsList::new(jail.clone()));
    registry.register(fs_search::FsSearch::new(jail));
    registry
}
