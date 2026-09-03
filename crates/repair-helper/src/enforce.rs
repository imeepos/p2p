//! 执法接线：每次 tools/call 先经 repair-enforce 分级后才放行/拒绝。
//!
//! 语义（remote-support-plan.md §3.4）：发送侧打标、helper 侧重判，不一致以
//! helper 为准；scope 由参数注入（T26 换 ticket 来源）；本批四只读工具恒判
//! read 放行；被拒调用返回带原因的工具错误。shell_exec 在 fix scope 白名单
//! 命中时经 [GateOutcome::NeedApproval] 路由到工具内审批（T23b）。
//!
//! 只消费 repair-enforce 冻结契约（[Enforcer]/[Verdict]/[Scope]/[ShellWhitelist]），
//! 不修改其任何接口。

use repair_enforce::{risk, Enforcer, Risk, Scope, ShellWhitelist, ToolCall, Verdict};
use serde_json::Value;

/// host 侧执法配置：scope + shell 白名单闭集。
#[derive(Clone)]
pub struct Enforcement {
    scope: Scope,
    whitelist: ShellWhitelist,
}

impl Enforcement {
    /// scope 参数化注入；白名单数据由 T24/配置注入（本轮缺省空闭集=全拒）。
    pub fn new(scope: Scope, whitelist: ShellWhitelist) -> Self {
        Self { scope, whitelist }
    }

    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// 宿主门裁决：硬拒（红线段/白名单未命中/参数不匹配/diag 档）给出原因；
    /// 放行给出风险档；fix scope 下 write/danger 应路由审批。
    pub fn gate(&self, tool: &str, arguments: &Value) -> GateOutcome {
        let call = build_call(tool, arguments);
        match Enforcer::new(self.scope, &self.whitelist).evaluate(&call) {
            Verdict::Allow => GateOutcome::Allow(risk::classify(&call)),
            Verdict::Redline(rl) => GateOutcome::Deny(format!("redline denied: {}", rl.reason())),
            Verdict::WhitelistDenied => GateOutcome::Deny(self.whitelist_deny_reason(&call)),
            Verdict::ScopeDenied => {
                GateOutcome::Deny("denied: write/danger call outside diag scope".into())
            }
            // 仅 shell_exec 由工具内审批承接；其余工具的审批流程尚未接线 -> 拒。
            Verdict::NeedApproval(risk) if tool == "shell_exec" => GateOutcome::NeedApproval(risk),
            Verdict::NeedApproval(_) => {
                GateOutcome::Deny("denied: approval flow not wired for this tool".into())
            }
        }
    }

    /// 白名单拒绝细化原因：区分「程序不在闭集」与「参数模式不匹配」。
    fn whitelist_deny_reason(&self, call: &ToolCall) -> String {
        if call.tool != "shell_exec" {
            return "denied: command not in closed whitelist".into();
        }
        let argv = call.shell_argv();
        match argv.first() {
            None => "denied: empty argv".into(),
            Some(program) if self.whitelist.find(&argv).is_none() => {
                format!("denied: '{program}' not in closed whitelist")
            }
            Some(program) => {
                format!("denied: argument pattern does not match whitelist rule for '{program}'")
            }
        }
    }

    /// 完整执法裁决：放行返回 Ok(风险档)，拒绝返回 Err(带原因)。
    pub fn evaluate(&self, tool: &str, arguments: &Value) -> Result<Risk, String> {
        let call = build_call(tool, arguments);
        match Enforcer::new(self.scope, &self.whitelist).evaluate(&call) {
            Verdict::Allow => Ok(risk::classify(&call)),
            Verdict::Redline(rl) => Err(format!("redline denied: {}", rl.reason())),
            Verdict::WhitelistDenied => Err("denied: command not in closed whitelist".into()),
            Verdict::ScopeDenied => Err("denied: write/danger call outside diag scope".into()),
            Verdict::NeedApproval(_) => {
                Err("denied: approval required for write/danger call".into())
            }
        }
    }

    /// 风险档判定（审计用，独立于放行与否）。
    pub fn classify(&self, tool: &str, arguments: &Value) -> Risk {
        risk::classify(&build_call(tool, arguments))
    }
}

/// 独立分类入口（无 enforcement 的宿主也审计风险档）。
pub fn classify(tool: &str, arguments: &Value) -> Risk {
    risk::classify(&build_call(tool, arguments))
}

/// 宿主门裁决结果：放行（带风险档）/ 需审批（fix scope write/danger）/ 拒绝（带原因）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateOutcome {
    /// 放行，风险档供审计。
    Allow(Risk),
    /// fix scope 下 write/danger：应路由到审批（shell_exec 由工具内审批承接）。
    NeedApproval(Risk),
    /// 无条件/参数级拒绝，带原因（不 spawn）。
    Deny(String),
}

/// 风险档机器名（审计事件字段）。
pub fn risk_name(risk: Risk) -> &'static str {
    match risk {
        Risk::Read => "read",
        Risk::Write => "write",
        Risk::Danger => "danger",
    }
}

/// 把 MCP 参数对象拍平为 enforce 的 ToolCall（标量取值，数组按空格拼接）。
fn build_call(tool: &str, arguments: &Value) -> ToolCall {
    let params = arguments
        .as_object()
        .map(|map| {
            map.iter()
                .map(|(k, v)| (k.clone(), scalar_text(v)))
                .collect()
        })
        .unwrap_or_default();
    ToolCall {
        tool: tool.to_string(),
        params,
    }
}

/// 参数值拍平：字符串原样；字符串数组按空格拼接（shell_exec argv 语义，
/// 与 repair-enforce util::split_words 词法一致，P0b 简单 token 约定）；
/// null 置空；其余标量 compact 化。
fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(items) => {
            let parts: Vec<&str> = items.iter().filter_map(Value::as_str).collect();
            parts.join(" ")
        }
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repair_enforce::risk::Risk;

    fn args(kv: &[(&str, &str)]) -> Value {
        let mut map = serde_json::Map::new();
        for (k, v) in kv {
            map.insert(k.to_string(), Value::String(v.to_string()));
        }
        Value::Object(map)
    }

    #[test]
    fn four_read_tools_allowed_in_both_scopes() {
        for scope in [Scope::Diag, Scope::Fix] {
            for tool in ["sys_snapshot", "fs_read", "fs_list", "fs_search"] {
                let e = Enforcement::new(scope, ShellWhitelist::empty());
                let verdict = e.evaluate(tool, &args(&[("path", "a.txt")]));
                assert_eq!(verdict, Ok(Risk::Read), "{tool} under {scope:?}");
            }
        }
    }

    #[test]
    fn write_tool_denied_in_diag_with_reason() {
        let e = Enforcement::new(Scope::Diag, ShellWhitelist::empty());
        let err = e
            .evaluate("fs_write", &args(&[("path", "a.txt")]))
            .unwrap_err();
        assert!(err.contains("diag"), "unexpected reason: {err}");
        assert_eq!(
            e.classify("fs_write", &args(&[("path", "a.txt")])),
            Risk::Write
        );
    }

    #[test]
    fn write_tool_denied_in_fix_until_approval() {
        let e = Enforcement::new(Scope::Fix, ShellWhitelist::empty());
        let err = e
            .evaluate("fs_write", &args(&[("path", "a.txt")]))
            .unwrap_err();
        assert!(err.contains("approval"), "unexpected reason: {err}");
    }

    #[test]
    fn unknown_tool_is_danger_and_denied_in_diag() {
        let e = Enforcement::new(Scope::Diag, ShellWhitelist::empty());
        assert_eq!(e.classify("totally_unknown", &args(&[])), Risk::Danger);
        assert!(e.evaluate("totally_unknown", &args(&[])).is_err());
    }

    #[test]
    fn credential_path_hits_redline() {
        let e = Enforcement::new(Scope::Diag, ShellWhitelist::empty());
        let err = e
            .evaluate("fs_read", &args(&[("path", "/Users/x/.ssh/id_rsa")]))
            .unwrap_err();
        assert!(err.contains("redline"), "unexpected reason: {err}");
    }

    #[test]
    fn risk_names_stable() {
        assert_eq!(risk_name(Risk::Read), "read");
        assert_eq!(risk_name(Risk::Write), "write");
        assert_eq!(risk_name(Risk::Danger), "danger");
    }

    #[test]
    fn classify_standalone_matches_enforced() {
        let v = args(&[("path", "a.txt")]);
        assert_eq!(classify("fs_read", &v), Risk::Read);
        assert_eq!(classify("fs_write", &v), Risk::Write);
    }

    fn argv_arg(list: &[&str]) -> Value {
        let list = list
            .iter()
            .map(|s| Value::String(s.to_string()))
            .collect::<Vec<_>>();
        let mut map = serde_json::Map::new();
        map.insert("argv".into(), Value::Array(list));
        Value::Object(map)
    }

    fn whitelist_echo() -> ShellWhitelist {
        let mut w = ShellWhitelist::empty();
        w.add(repair_enforce::whitelist::ShellRule::new(
            "echo",
            vec![repair_enforce::whitelist::ArgPat::Any],
        ));
        w
    }

    #[test]
    fn shell_exec_fix_whitelist_hit_routes_to_approval() {
        let e = Enforcement::new(Scope::Fix, whitelist_echo());
        let outcome = e.gate("shell_exec", &argv_arg(&["echo", "hi"]));
        assert_eq!(
            outcome,
            GateOutcome::NeedApproval(Risk::Write),
            "{outcome:?}"
        );
    }

    #[test]
    fn shell_exec_empty_whitelist_denied_with_program_name() {
        let e = Enforcement::new(Scope::Fix, ShellWhitelist::empty());
        let reason = match e.gate("shell_exec", &argv_arg(&["echo", "hi"])) {
            GateOutcome::Deny(reason) => reason,
            other => panic!("expected deny, got {other:?}"),
        };
        assert!(reason.contains("echo"), "unexpected: {reason}");
        assert!(
            reason.contains("not in closed whitelist"),
            "unexpected: {reason}"
        );
    }

    #[test]
    fn shell_exec_diag_scope_denied_no_approval() {
        let e = Enforcement::new(Scope::Diag, whitelist_echo());
        let outcome = e.gate("shell_exec", &argv_arg(&["echo", "hi"]));
        assert!(matches!(outcome, GateOutcome::Deny(_)), "{outcome:?}");
    }

    #[test]
    fn shell_exec_redline_wins_over_whitelist() {
        let mut w = whitelist_echo();
        w.add(repair_enforce::whitelist::ShellRule::new(
            "rm",
            vec![
                repair_enforce::whitelist::ArgPat::prefix("-"),
                repair_enforce::whitelist::ArgPat::Any,
                repair_enforce::whitelist::ArgPat::Any,
            ],
        ));
        let e = Enforcement::new(Scope::Fix, w);
        let reason = match e.gate("shell_exec", &argv_arg(&["rm", "-rf", "/"])) {
            GateOutcome::Deny(reason) => reason,
            other => panic!("expected deny, got {other:?}"),
        };
        assert!(reason.contains("redline"), "unexpected: {reason}");
    }
}
