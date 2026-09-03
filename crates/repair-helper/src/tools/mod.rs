//! 工具面装配：只读四件套（T23）+ shell_exec 执行面（T23b）与注册入口。
//!
//! 风险档判定在 repair-enforce RISK_RULES（T23 四只读恒判 read；shell_exec
//! 按命令重判 write/danger），见 remote-support-plan.md §3.5 工具表。

pub mod approval;
pub mod fs_list;
pub mod fs_read;
pub mod fs_search;
pub mod glob;
pub mod shell_exec;
pub mod sys_snapshot;

#[cfg(test)]
mod shell_exec_tests;
#[cfg(test)]
mod shell_host_tests;

use crate::jail::PathJail;
use crate::ToolRegistry;

/// 只读四件套注册表（T23）：sys_snapshot/fs_read/fs_list/fs_search。
pub fn read_only_registry(jail: PathJail) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(sys_snapshot::SysSnapshot::new());
    registry.register(fs_read::FsRead::new(jail.clone()));
    registry.register(fs_list::FsList::new(jail.clone()));
    registry.register(fs_search::FsSearch::new(jail));
    registry
}

/// 全量注册表：只读四件套 + shell_exec（T23b）。
pub fn helper_registry(jail: PathJail, shell: shell_exec::ShellExec) -> ToolRegistry {
    let mut registry = read_only_registry(jail);
    registry.register(shell);
    registry
}
