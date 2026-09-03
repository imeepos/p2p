//! 执法接线：每次 tools/call 先经 repair-enforce 分级后才放行/拒绝。
//!
//! 语义（remote-support-plan.md §3.4）：发送侧打标、helper 侧重判，不一致以
//! helper 为准；scope 由参数注入（T26 换 ticket 来源）；本批四只读工具恒判
//! read 放行；被拒调用返回带原因的工具错误。
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
    /// scope 参数化注入；白名单本轮为空闭集（T24 填充数据）。
    pub fn new(scope: Scope, whitelist: ShellWhitelist) -> Self {
        Self { scope, whitelist }
    }

    pub fn scope(&self) -> Scope {
        self.scope
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

/// 风险档机器名（审计事件字段）。
pub fn risk_name(risk: Risk) -> &'static str {
    match risk {
        Risk::Read => "read",
        Risk::Write => "write",
        Risk::Danger => "danger",
    }
}

/// 把 MCP 参数对象拍平为 enforce 的 ToolCall（标量取值，复合值 compact 化）。
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

fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
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
}
