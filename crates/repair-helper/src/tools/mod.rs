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
use crate::session_report::SessionReport;
use crate::{AuditSink, ToolRegistry};

/// stdio 形态无票据，session_report 以本常量标识本机会话（runner-integration §7）。
pub const STDIO_TICKET_ID: &str = "stdio-local";

/// 只读四件套注册表（T23）：sys_snapshot/fs_read/fs_list/fs_search。
pub fn read_only_registry(jail: PathJail) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(sys_snapshot::SysSnapshot::new());
    registry.register(fs_read::FsRead::new(jail.clone()));
    registry.register(fs_list::FsList::new(jail.clone()));
    registry.register(fs_search::FsSearch::new(jail));
    registry
}

/// 全量注册表：只读四件套 + shell_exec（T23b）+ session_report。
/// audit 必须与 Host::guarded 同源，session_report 才能导出全部调用记录；
/// p2p 形态按票据逐例装配（p2p.rs host_for），工单号来自票据 payload。
pub fn helper_registry(
    jail: PathJail,
    shell: shell_exec::ShellExec,
    audit: AuditSink,
) -> ToolRegistry {
    let mut registry = read_only_registry(jail);
    registry.register(shell);
    registry.register(SessionReport::new(audit, STDIO_TICKET_ID));
    registry
}

#[cfg(test)]
mod registry_tests;
