//! shell 白名单闭集：argv[0] 白名单 + 参数模式校验，闭集外一律拒。
//!
//! 语义（remote-support-plan.md §3.4）：shell_exec 只能执行白名单内的程序，
//! 且参数必须匹配该程序声明的模式序列，任何不在闭集内的组合直接拒绝。
//! 清单数据本批为空（[ShellWhitelist::empty] 拒全部），由 T24 填充三类
//! playbook 命令并集。
//!
//! 匹配约定：程序名大小写不敏感（Windows 语义），支持裸名与全路径 argv[0]
//! （比较裸名时剥离目录与已知可执行扩展名）；参数模式逐位匹配、超长参数
//! 一律拒；所有字符串比较大小写不敏感。

/// 单条参数模式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgPat {
    /// 任意参数都接受。
    Any,
    /// 必须与该参数完全相等（大小写不敏感）。
    Exact(String),
    /// 必须以该前缀开头（大小写不敏感），如 "--path="。
    Prefix(String),
}

impl ArgPat {
    pub fn exact(s: &str) -> Self {
        Self::Exact(s.to_string())
    }

    pub fn prefix(s: &str) -> Self {
        Self::Prefix(s.to_string())
    }

    fn matches(&self, arg: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(e) => arg.eq_ignore_ascii_case(e),
            Self::Prefix(p) => arg
                .to_ascii_lowercase()
                .starts_with(&p.to_ascii_lowercase()),
        }
    }
}

/// 一条可执行规则：程序（argv[0]）+ 位置参数模式序列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellRule {
    /// 程序名（裸名或全路径）。
    pub program: String,
    /// argv[1..] 的位置参数模式；长度即允许的最大参数个数。
    pub args: Vec<ArgPat>,
}

impl ShellRule {
    pub fn new(program: &str, args: Vec<ArgPat>) -> Self {
        Self {
            program: program.to_string(),
            args,
        }
    }

    fn program_matches(&self, argv0: &str) -> bool {
        let rule = self.program.to_ascii_lowercase();
        let argv0_lower = argv0.to_ascii_lowercase();
        rule == argv0_lower || base_name(&rule) == base_name(&argv0_lower)
    }
}

/// shell 白名单闭集。规则为空时闭集为空，一切调用拒绝。
#[derive(Debug, Clone, Default)]
pub struct ShellWhitelist {
    rules: Vec<ShellRule>,
}

impl ShellWhitelist {
    /// 空清单：本轮由 T24 填充数据。
    pub fn empty() -> Self {
        Self::default()
    }

    /// 追加规则（T24 填充与测试构造用）。
    pub fn add(&mut self, rule: ShellRule) -> &mut Self {
        self.rules.push(rule);
        self
    }

    /// 当前规则清单。
    pub fn rules(&self) -> &[ShellRule] {
        &self.rules
    }

    /// 闭集匹配：argv[0] 命中白名单且参数逐位满足模式才放行，其余一律拒。
    pub fn is_allowed(&self, argv: &[String]) -> bool {
        let Some(rule) = self.find(argv) else {
            return false;
        };
        argv.len() - 1 <= rule.args.len()
            && argv[1..]
                .iter()
                .zip(rule.args.iter())
                .all(|(arg, pat)| pat.matches(arg))
    }

    /// 按 argv[0] 找规则（大小写不敏感，支持裸名/全路径）。
    pub fn find(&self, argv: &[String]) -> Option<&ShellRule> {
        let argv0 = argv.first()?;
        self.rules.iter().find(|r| r.program_matches(argv0))
    }
}

/// 剥离目录与已知可执行扩展名，得到比较用裸名。
fn base_name(argv0: &str) -> String {
    let tail = argv0.rsplit(['/', '\\']).next().unwrap_or(argv0);
    let lower = tail.to_ascii_lowercase();
    for ext in [".exe", ".cmd", ".bat", ".ps1", ".com"] {
        if let Some(stem) = lower.strip_suffix(ext) {
            return stem.to_string();
        }
    }
    lower
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn whitelist_with_common_rules() -> ShellWhitelist {
        let mut w = ShellWhitelist::empty();
        w.add(ShellRule::new(
            "tasklist",
            vec![
                ArgPat::exact("/FI"),
                ArgPat::Any,
                ArgPat::exact("/FO"),
                ArgPat::exact("csv"),
            ],
        ));
        w.add(ShellRule::new("whoami", vec![]));
        w.add(ShellRule::new("ipconfig", vec![ArgPat::Any]));
        w.add(ShellRule::new(
            "C:\\Windows\\System32\\chkdsk.exe",
            vec![ArgPat::prefix("/f"), ArgPat::prefix("C:")],
        ));
        w
    }

    #[test]
    fn empty_closed_set_rejects_everything() {
        let w = ShellWhitelist::empty();
        assert!(!w.is_allowed(&argv(&["tasklist"])));
        assert!(!w.is_allowed(&argv(&["whoami"])));
        assert!(!w.is_allowed(&argv(&[])));
    }

    #[test]
    fn program_in_whitelist_with_matching_args() {
        let w = whitelist_with_common_rules();
        assert!(w.is_allowed(&argv(&["whoami"])));
        assert!(w.is_allowed(&argv(&[
            "tasklist",
            "/FI",
            "IMAGENAME eq svchost.exe",
            "/FO",
            "csv"
        ])));
        assert!(w.is_allowed(&argv(&["ipconfig", "/all"])));
    }

    #[test]
    fn program_outside_closed_set_rejected() {
        let w = whitelist_with_common_rules();
        assert!(!w.is_allowed(&argv(&["notepad"])));
        assert!(!w.is_allowed(&argv(&["cmd.exe", "/c", "whoami"])));
    }

    #[test]
    fn argument_pattern_mismatch_rejected() {
        let w = whitelist_with_common_rules();
        assert!(!w.is_allowed(&argv(&["tasklist", "/FI", "NAME", "EXTRA"])));
        assert!(!w.is_allowed(&argv(&["tasklist", "/FO", "csv"])));
        assert!(!w.is_allowed(&argv(&["whoami", "/all"])));
    }

    #[test]
    fn too_many_arguments_rejected() {
        let w = whitelist_with_common_rules();
        assert!(!w.is_allowed(&argv(&["whoami", "a", "b"])));
        assert!(!w.is_allowed(&argv(&["ipconfig", "/a", "/b"])));
    }

    #[test]
    fn case_insensitive_program_and_args() {
        let w = whitelist_with_common_rules();
        assert!(w.is_allowed(&argv(&["WHOAMI"])));
        assert!(w.is_allowed(&argv(&["WhoAmI"])));
        assert!(w.is_allowed(&argv(&["TASKLIST", "/fi", "x", "/fo", "CSV"])));
    }

    #[test]
    fn full_path_and_extension_variants() {
        let w = whitelist_with_common_rules();
        assert!(w.is_allowed(&argv(&["C:\\Windows\\System32\\whoami.exe"])));
        assert!(w.is_allowed(&argv(&["C:\\Windows\\System32\\CHKDSK.EXE", "/f", "C:"])));
        assert!(w.is_allowed(&argv(&["chkdsk", "/f", "c:"])));
        assert!(!w.is_allowed(&argv(&["chkdsk", "/x", "c:"])));
    }
}
