//! repair-playbook：playbook 结构化 markdown 解析与校验（agent 无关格式 v1，RS P0b T25）。
//!
//! 一份 playbook 覆盖一类问题，同时包含诊断（只读）与修复（写类）步骤；服务侧
//! runner 与 shell 白名单均消费本格式。
//!
//! # 格式规范 v1（与 docs/playbooks/README.md 双写，改动必须两处同步）
//!
//! ## 定位
//!
//! 除推荐 runner 字段外，playbook 不得出现任何 runner 专有绑定（DSH/Codex/Claude
//! 等均禁止）；推荐 runner 允许留空或填写枚举字符串。格式面向任意 agent 的 MCP
//! 工具面（remote-support-design.md §9）。
//!
//! ## 文件结构
//!
//! 文件为 UTF-8 编码的受控 markdown 子集，结构固定：
//!
//! ```text
//! # Playbook: <名称>                     # H1，必填，必须是首个非空行
//!
//! - 名称: <名称>                          # 必填，须与 H1 一致
//! - 问题类别: <类别标识符>                 # 必填，如 performance-slow
//! - 推荐 runner: <枚举字符串或留空>       # 可选
//! - 前置条件: <项>                        # 必填，至少一项；多项目占一行或用二级列表逐项
//!   - <前置条件项>
//! - 备注: <文本>                          # 可选，可重复
//!
//! ## 红线清单                            # 必填，至少一项
//!
//! - <整体红线项>
//!
//! ## 步骤 <N> <标题(可选)>               # N 从 1 起严格连续递增
//!
//! - 说明: <本步做什么>                    # 必填
//! - 工具: <tool 名>                      # 必填，取自已冻结 P0b 工具面
//! - 参数: <参数要点>                      # fs_read/fs_list/fs_search 必填；其余可省略
//! - 命令: <shell 命令单行>                # 工具为 shell_exec 时必填
//! - 风险档: read|write|danger             # 工具为 shell_exec 时必填
//! - 验收: <验收命令或可判定标准>          # 必填
//! - 红线: <该步红线>                      # 必填，可重复形成多条
//! - 备注: <文本>                          # 可选，可重复
//! ```
//!
//! ## 规则
//!
//! - H1 必须是文档首个非空行；`## 红线清单` 与 `## 步骤 <N>` 是仅有的两种二级章节；
//! - 步骤号从 1 起严格连续递增，缺失/非法/跳号均为错误；
//! - 工具名必须来自 P0b 工具面闭集：sys_snapshot、fs_read、fs_list、fs_search、
//!   shell_exec、session_report（remote-support-plan.md §3.5）；未知工具引用为错误；
//! - shell_exec 步骤必须给出单行 `命令`（shell 命令原文）并标注 `风险档`
//!   （read=只读无副作用；write=可写需审批；danger=高危需审批）；命令/风险档字段
//!   只允许出现在 shell_exec 步骤；
//! - 每个步骤必须有 `验收`（验收命令或可判定标准）与至少一条 `红线`；
//! - 整体红线清单与前置条件至少各一项；所有字段值单行表达（不跨行）；`红线`/
//!   `备注` 用重复键表达多条；
//! - 出现未允许的字段为错误。所有校验失败返回带行号的 [ParseError]，禁止静默忽略；
//! - 写类命令的风险档由 author 逐步骤标注，helper 侧仍独立重判（以本地重判为准，
//!   remote-support-plan.md §3.4）。
//!
//! ## shell 命令清单导出
//!
//! 解析结果经 [Playbook::shell_commands] 聚合全部步骤中的 shell 命令（含风险档与
//! 步骤号），[shell_union] 跨多份 playbook 按出现顺序求并集，供 shell 白名单闭集
//! （remote-support-plan.md Q7 / T24）直接消费。
//!
//! # 日志
//!
//! 解析本身完全确定性：失败一律以 [ParseError] 携带行号返回，不产生默认日志；
//! 目录装载 [load_dir] 的失败路径逐个发出 tracing::warn!（宿主经 p2p-log 统一初始化）。
//!
//! ```rust
//! use repair_playbook::parse;
//! let pb = parse(Some("sample.md"), repair_playbook::SAMPLE);
//! assert!(pb.is_ok());
//! ```

use std::fmt;
use std::path::PathBuf;

pub mod emit;
pub mod parse;

mod finalize;
mod front;
mod steps;
mod walker;

#[cfg(test)]
mod tests;

pub use emit::shell_union;
pub use parse::{load_dir, parse};

/// P0b 已冻结工具面闭集（remote-support-plan.md §3.5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    SysSnapshot,
    FsRead,
    FsList,
    FsSearch,
    ShellExec,
    SessionReport,
}

impl Tool {
    /// 工具的 playbook 字段名。
    pub fn as_str(&self) -> &'static str {
        match self {
            Tool::SysSnapshot => "sys_snapshot",
            Tool::FsRead => "fs_read",
            Tool::FsList => "fs_list",
            Tool::FsSearch => "fs_search",
            Tool::ShellExec => "shell_exec",
            Tool::SessionReport => "session_report",
        }
    }

    /// 按字段名解析；未知名称返回 None（解析器据此报「未知工具引用」）。
    pub fn from_name(name: &str) -> Option<Tool> {
        match name {
            "sys_snapshot" => Some(Tool::SysSnapshot),
            "fs_read" => Some(Tool::FsRead),
            "fs_list" => Some(Tool::FsList),
            "fs_search" => Some(Tool::FsSearch),
            "shell_exec" => Some(Tool::ShellExec),
            "session_report" => Some(Tool::SessionReport),
            _ => None,
        }
    }

    /// 该工具是否必须携带 `参数` 要点（sys_snapshot/session_report 无参）。
    pub fn needs_params(&self) -> bool {
        matches!(self, Tool::FsRead | Tool::FsList | Tool::FsSearch)
    }
}

/// 风险三档，语义与 repair-enforce 的 Risk 对齐（T24 接线时互转）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    /// 只读，无副作用。
    Read,
    /// 可写（含删除单文件等），fix scope 下需审批。
    Write,
    /// 高危写操作，fix scope 下需审批。
    Danger,
}

impl RiskLevel {
    /// 风险档的 playbook 字段值。
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Read => "read",
            RiskLevel::Write => "write",
            RiskLevel::Danger => "danger",
        }
    }

    /// 按字段值解析；非法值返回 None（解析器据此报「非法风险档」）。
    pub fn from_name(name: &str) -> Option<RiskLevel> {
        match name {
            "read" => Some(RiskLevel::Read),
            "write" => Some(RiskLevel::Write),
            "danger" => Some(RiskLevel::Danger),
            _ => None,
        }
    }
}

/// 一次 shell 调用：命令原文 + 风险档 + 归属步骤与源行号。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellInvocation {
    /// 归属步骤号。
    pub step: u32,
    /// shell 命令原文（单行）。
    pub command: String,
    /// 风险档（read/write/danger）。
    pub risk: RiskLevel,
    /// 源文档行号（1-based）。
    pub line: usize,
}

/// playbook 中的单个步骤。
#[derive(Debug, Clone)]
pub struct Step {
    /// 步骤号（1 起严格连续）。
    pub number: u32,
    /// 步骤标题（`## 步骤 <N> <标题>`，可省略）。
    pub title: Option<String>,
    /// 说明：本步做什么。
    pub description: String,
    /// 工具名（P0b 闭集内）。
    pub tool: Tool,
    /// 参数要点（fs_* 必填，其余可省略）。
    pub params: Option<String>,
    /// shell 调用（仅 shell_exec 步骤）。
    pub shell: Option<ShellInvocation>,
    /// 验收：验收命令或可判定标准。
    pub acceptance: String,
    /// 该步红线（至少一条）。
    pub redlines: Vec<String>,
    /// 该步备注（可重复）。
    pub notes: Vec<String>,
    /// 步骤标题所在源行号（1-based）。
    pub line: usize,
}

/// 一份解析完成的 playbook。
#[derive(Debug, Clone)]
pub struct Playbook {
    /// 名称（与 H1 一致）。
    pub name: String,
    /// 问题类别标识符。
    pub category: String,
    /// 推荐 runner；留空或省略表示不限。
    pub runner: Option<String>,
    /// 前置条件（至少一项）。
    pub prerequisites: Vec<String>,
    /// 文档级备注（可重复）。
    pub notes: Vec<String>,
    /// 整体红线清单（至少一项）。
    pub redlines: Vec<String>,
    /// 诊断与修复步骤。
    pub steps: Vec<Step>,
}

/// 目录装载结果：文件路径 + 解析完成的 playbook。
#[derive(Debug, Clone)]
pub struct LoadedPlaybook {
    /// playbook 文件路径。
    pub path: PathBuf,
    /// 解析完成的 playbook。
    pub playbook: Playbook,
}

/// 解析/校验错误：带来源（可选）与 1-based 行号，禁静默忽略。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// 来源标识（目录装载时为文件名，字符串解析时为 None）。
    pub source: Option<String>,
    /// 1-based 行号；0 表示文件级错误（非行级）。
    pub line: usize,
    /// 错误描述。
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.source, self.line) {
            (Some(src), 0) => write!(f, "{src}: {}", self.message),
            (Some(src), line) => write!(f, "{src}:{line}: {}", self.message),
            (None, 0) => write!(f, "{}", self.message),
            (None, line) => write!(f, "line {line}: {}", self.message),
        }
    }
}

impl std::error::Error for ParseError {}

/// 解析器自测样例（也用于 crate 文档示例）。
pub const SAMPLE: &str = r#"# Playbook: 测试样本

- 名称: 测试样本
- 问题类别: sample-category
- 推荐 runner:
- 前置条件:
  - Windows 10/11
  - 已授权 fix scope
- 备注: P0b 真机演练时校准

## 红线清单

- 禁止 format
- 禁止批量删除用户目录

## 步骤 1 采集快照

- 说明: 采集系统基线。
- 工具: sys_snapshot
- 参数: 无
- 验收: 快照返回无错误。
- 红线: 只读，不修改系统状态。

## 步骤 2 查询高 CPU 进程

- 说明: 列出高 CPU 进程。
- 工具: shell_exec
- 命令: Get-Process | Sort-Object CPU -Descending | Select-Object -First 5
- 风险档: read
- 验收: 输出进程表。
- 红线: 不结束任何进程。
- 备注: 校准点
"#;
