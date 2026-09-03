//! 风险分级：按 tool 名 + 参数特征把一次工具调用预分类为 read/write/danger 三档。
//!
//! 判定规则全部数据化在 [RISK_RULES] 表中（tool 名 + 参数谓词 + 风险档），
//! 新增工具或调档只改表，不在代码里散落分支。未知 tool 一律按最高档 danger
//! 处理（安全默认：宁严勿宽）。
//!
//! 语义参考：remote-support-plan.md §3.4——发送侧打标、接收侧（helper）独立
//! 重判，不一致以本地重判为准；本表就是 helper 侧重判的实现。

use crate::util;

/// 风险三档。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    /// 只读，无副作用，任何 scope 均可直接执行。
    Read,
    /// 可写（含删除单文件等），fix scope 下需审批。
    Write,
    /// 高危（敏感命令/未知工具），fix scope 下需审批。
    Danger,
}

/// 一次工具调用的规范化描述：tool 名 + 参数键值对。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub tool: String,
    pub params: Vec<(String, String)>,
}

impl ToolCall {
    /// 取指定参数名的值；参数缺失返回 None。
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// shell_exec 的命令行：取 argv 参数按空格分词（调用侧以数组语义拼接）。
    pub fn shell_argv(&self) -> Vec<String> {
        self.param("argv")
            .map(util::split_words)
            .unwrap_or_default()
    }

    /// 全部参数值拼接成的匹配文本。
    pub fn hay(&self) -> String {
        self.params.iter().map(|(_, v)| v.as_str()).collect()
    }
}

/// 参数特征谓词：规则命中条件。
#[derive(Debug, Clone, Copy)]
pub enum ParamPred {
    /// 不限定参数。
    Any,
    /// 任一参数值命中关键词列表（词边界匹配，见 [crate::util::contains_any]）。
    ContainsAnyOf(&'static [&'static str]),
}

/// 一条风险判定规则。
#[derive(Debug, Clone, Copy)]
pub struct RiskRule {
    /// tool 名，匹配时小写归一。
    pub tool: &'static str,
    /// 参数命中条件。
    pub when: ParamPred,
    /// 命中后赋予的风险档。
    pub risk: Risk,
    /// 规则出处/含义说明。
    pub note: &'static str,
}

/// shell 命令中触发 danger 档的敏感词（粗分，细化判定在 redline 模块）。
pub static SHELL_SENSITIVE_KEYWORDS: &[&str] = &[
    "format",
    "mkfs",
    "fdisk",
    "diskpart",
    "wipefs",
    "shred",
    "dd",
    "rm",
    "del",
    "erase",
    "rmdir",
    "rd",
    "remove-item",
    "gpg",
    "gpg2",
    "age",
    "cryptsetup",
    "luks",
    "bitlocker",
    "defender",
    "windefend",
    "mppreference",
    "mcafee",
    "norton",
    "kaspersky",
    "password",
    "credentials",
    "id_rsa",
    "id_ed25519",
    ".ssh",
    "secret",
];

/// 风险判定规则表（数据化承载，新增/调档只改此表）。
pub static RISK_RULES: &[RiskRule] = &[
    RiskRule {
        tool: "sys_snapshot",
        when: ParamPred::Any,
        risk: Risk::Read,
        note: "系统快照，只读诊断入口",
    },
    RiskRule {
        tool: "fs_read",
        when: ParamPred::Any,
        risk: Risk::Read,
        note: "文件读取，监狱内只读",
    },
    RiskRule {
        tool: "fs_list",
        when: ParamPred::Any,
        risk: Risk::Read,
        note: "目录列举，监狱内只读",
    },
    RiskRule {
        tool: "fs_search",
        when: ParamPred::Any,
        risk: Risk::Read,
        note: "内容搜索，监狱内只读",
    },
    RiskRule {
        tool: "proc_query",
        when: ParamPred::Any,
        risk: Risk::Read,
        note: "进程查询，只读",
    },
    RiskRule {
        tool: "svc_query",
        when: ParamPred::Any,
        risk: Risk::Read,
        note: "服务查询，只读",
    },
    RiskRule {
        tool: "net_diag",
        when: ParamPred::Any,
        risk: Risk::Read,
        note: "网络诊断，只读",
    },
    RiskRule {
        tool: "session_report",
        when: ParamPred::Any,
        risk: Risk::Read,
        note: "执行记录导出，只读",
    },
    RiskRule {
        tool: "fs_write",
        when: ParamPred::Any,
        risk: Risk::Write,
        note: "写文件，需审批",
    },
    RiskRule {
        tool: "fs_edit",
        when: ParamPred::Any,
        risk: Risk::Write,
        note: "修改文件，需审批",
    },
    RiskRule {
        tool: "backup_point",
        when: ParamPred::Any,
        risk: Risk::Write,
        note: "还原点/快照，需审批",
    },
    RiskRule {
        tool: "fs_delete",
        when: ParamPred::Any,
        risk: Risk::Danger,
        note: "删除文件，P1 预留，高危",
    },
    RiskRule {
        tool: "shell_exec",
        when: ParamPred::Any,
        risk: Risk::Write,
        note: "shell 执行基础档（具体命令再判）",
    },
    RiskRule {
        tool: "shell_exec",
        when: ParamPred::ContainsAnyOf(SHELL_SENSITIVE_KEYWORDS),
        risk: Risk::Danger,
        note: "shell 执行命中敏感词升 danger",
    },
];

/// 对一次工具调用判定风险档：表中命中规则的最高档；未知 tool 返回 danger。
pub fn classify(call: &ToolCall) -> Risk {
    let tool = call.tool.to_ascii_lowercase();
    let hay = call.hay();
    RISK_RULES
        .iter()
        .filter(|r| r.tool == tool)
        .filter(|r| match r.when {
            ParamPred::Any => true,
            ParamPred::ContainsAnyOf(words) => util::contains_any(&hay, words),
        })
        .map(|r| r.risk)
        .max()
        .unwrap_or(Risk::Danger)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(tool: &str, params: &[(&str, &str)]) -> ToolCall {
        ToolCall {
            tool: tool.to_string(),
            params: params
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    #[test]
    fn read_tools_classify_read() {
        for t in [
            "sys_snapshot",
            "fs_read",
            "fs_list",
            "fs_search",
            "proc_query",
            "svc_query",
            "net_diag",
            "session_report",
        ] {
            assert_eq!(classify(&call(t, &[])), Risk::Read, "tool {}", t);
        }
    }

    #[test]
    fn write_tools_classify_write() {
        for t in ["fs_write", "fs_edit", "backup_point"] {
            assert_eq!(
                classify(&call(t, &[("path", "C:/a.txt")])),
                Risk::Write,
                "tool {}",
                t
            );
        }
    }

    #[test]
    fn shell_tool_default_write_but_sensitive_danger() {
        assert_eq!(
            classify(&call("shell_exec", &[("argv", "tasklist")])),
            Risk::Write
        );
        assert_eq!(
            classify(&call("shell_exec", &[("argv", "rm temp.txt")])),
            Risk::Danger
        );
    }

    #[test]
    fn unknown_tool_is_danger() {
        assert_eq!(classify(&call("totally_unknown", &[])), Risk::Danger);
    }

    #[test]
    fn case_insensitive_tool_name() {
        assert_eq!(classify(&call("FS_READ", &[])), Risk::Read);
        assert_eq!(
            classify(&call("Shell_Exec", &[("argv", "whoami")])),
            Risk::Write
        );
    }
}
