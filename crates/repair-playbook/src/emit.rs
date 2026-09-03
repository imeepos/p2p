//! 规范化输出与 shell 命令导出（供 runner 与 T24 白名单并集消费）。

use crate::{Playbook, ShellInvocation, Step};

impl Playbook {
    /// 输出规范化 markdown（可再次解析；语义与原文档一致，行号不保留）。
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Playbook: {}\n", self.name));
        out.push_str(&format!("- 名称: {}\n", self.name));
        out.push_str(&format!("- 问题类别: {}\n", self.category));
        if let Some(r) = self.runner.as_deref().filter(|r| !r.is_empty()) {
            out.push_str(&format!("- 推荐 runner: {r}\n"));
        }
        out.push_str("- 前置条件:\n");
        for p in &self.prerequisites {
            out.push_str(&format!("  - {p}\n"));
        }
        for n in &self.notes {
            out.push_str(&format!("- 备注: {n}\n"));
        }
        out.push('\n');
        out.push_str("## 红线清单\n");
        for r in &self.redlines {
            out.push_str(&format!("- {r}\n"));
        }
        for s in &self.steps {
            push_step(&mut out, s);
        }
        out
    }

    /// 聚合全部步骤的 shell 命令（按步骤顺序），含风险档与步骤号。
    pub fn shell_commands(&self) -> Vec<&ShellInvocation> {
        self.steps.iter().filter_map(|s| s.shell.as_ref()).collect()
    }
}

fn push_step(out: &mut String, s: &Step) {
    out.push('\n');
    match &s.title {
        Some(t) => out.push_str(&format!("## 步骤 {} {t}\n", s.number)),
        None => out.push_str(&format!("## 步骤 {}\n", s.number)),
    }
    out.push_str(&format!("- 说明: {}\n", s.description));
    out.push_str(&format!("- 工具: {}\n", s.tool.as_str()));
    if let Some(p) = &s.params {
        out.push_str(&format!("- 参数: {p}\n"));
    }
    if let Some(sh) = &s.shell {
        out.push_str(&format!("- 命令: {}\n", sh.command));
        out.push_str(&format!("- 风险档: {}\n", sh.risk.as_str()));
    }
    out.push_str(&format!("- 验收: {}\n", s.acceptance));
    for r in &s.redlines {
        out.push_str(&format!("- 红线: {r}\n"));
    }
    for n in &s.notes {
        out.push_str(&format!("- 备注: {n}\n"));
    }
}

impl ShellInvocation {
    /// argv[0] 近似：命令首 token（白名单 argv0 判定用，不做参数解析）。
    pub fn argv0(&self) -> &str {
        self.command.split_whitespace().next().unwrap_or("")
    }
}

/// 跨多份 playbook 求 shell 命令并集：按首次出现顺序去重，供 Q7 白名单并集直接消费。
pub fn shell_union<'a>(playbooks: &[&'a Playbook]) -> Vec<&'a str> {
    let mut union: Vec<&str> = Vec::new();
    for pb in playbooks {
        for cmd in pb.shell_commands() {
            if !union.contains(&cmd.command.as_str()) {
                union.push(cmd.command.as_str());
            }
        }
    }
    union
}
