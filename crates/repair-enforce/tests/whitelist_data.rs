//! T24 白名单闭集集成测试（crate 外部视角，dev-dependency repair-playbook）。
//! 1. 数据一致性：内嵌白名单命令集合 == 三类草案 shell_union 并集（机制性防漂移，
//!    草案改动即本测试红，提醒更新 whitelist_data::WHITELIST_TABLE）。
//! 2. 判定语义：闭集外拒绝、参数不匹配拒绝、管道/重定向/命令替换拒绝、
//!    红线优先于白名单、三类草案逐类命中路径（fix 下进审批流）。

use repair_enforce::{
    builtin, Enforcer, Redline, Risk, Scope, ShellDenyReason, ToolCall, Verdict, WHITELIST_TABLE,
};
use std::collections::BTreeSet;
use std::path::Path;

fn playbook_dir() -> std::path::PathBuf {
    // cargo test 的 CARGO_MANIFEST_DIR 指向 crates/repair-enforce，docs 在仓库根。
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/playbooks")
}

fn union_commands() -> Vec<String> {
    let loaded = repair_playbook::load_dir(&playbook_dir()).expect("三类草案应可解析");
    let pbs: Vec<&repair_playbook::Playbook> = loaded.iter().map(|l| &l.playbook).collect();
    repair_playbook::shell_union(&pbs)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn call(tool: &str, argv: &str) -> ToolCall {
    ToolCall {
        tool: tool.to_string(),
        params: vec![("argv".to_string(), argv.to_string())],
    }
}

fn argv(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

// ---- 数据一致性（需求 1/2：内嵌数据 == shell_union 并集，防漂移） ----

#[test]
fn embedded_table_matches_shell_union() {
    let union: BTreeSet<String> = union_commands().into_iter().collect();
    let embedded: BTreeSet<&str> = WHITELIST_TABLE.iter().map(|e| e.command).collect();
    assert_eq!(embedded.len(), union.len(), "内嵌命令数应等于并集命令数");
    for u in &union {
        assert!(
            embedded.contains(u.as_str()),
            "并集命令未登记进白名单表: {u}"
        );
    }
    for e in &embedded {
        assert!(union.contains(*e), "白名单表存在并集外的命令: {e}");
    }
}

#[test]
fn each_entry_program_is_argv0() {
    for e in WHITELIST_TABLE {
        let argv0 = e.command.split_whitespace().next().unwrap_or("");
        assert_eq!(e.program, argv0, "program 应为命令 argv[0]: {}", e.command);
    }
}

#[test]
fn builtin_rule_count_equals_executable_entries() {
    let w = builtin();
    let executable = WHITELIST_TABLE
        .iter()
        .filter(|e| !e.patterns.is_empty())
        .count();
    assert_eq!(w.rules().len(), executable, "规则数应等于非复合表项数");
}

// ---- 判定语义（需求 3/4：闭集外拒绝带原因） ----

#[test]
fn closed_set_rejects_unknown_program_with_reason() {
    let w = builtin();
    assert_eq!(
        w.deny_reason(&argv(&["notepad.exe", "a.txt"])),
        Some(ShellDenyReason::UnknownProgram)
    );
    assert_eq!(
        w.deny_reason(&argv(&["cmd.exe", "/c", "whoami"])),
        Some(ShellDenyReason::UnknownProgram)
    );
    assert!(!w.is_allowed(&argv(&["Get-Process"])));
}

#[test]
fn arg_mismatch_rejected_with_reason() {
    let w = builtin();
    // netsh winhttp <错词> proxy：show/reset 都不匹配
    assert_eq!(
        w.deny_reason(&argv(&["netsh", "winhttp", "foo", "proxy"])),
        Some(ShellDenyReason::ArgMismatch)
    );
    // Stop-Process 用未知开关
    assert_eq!(
        w.deny_reason(&argv(&["Stop-Process", "-PID", "1234", "-Force"])),
        Some(ShellDenyReason::ArgMismatch)
    );
    // 参数过多（超出模式长度）
    assert_eq!(
        w.deny_reason(&argv(&[
            "Dism.exe",
            "/Online",
            "/Cleanup-Image",
            "/StartComponentCleanup",
            "extra"
        ])),
        Some(ShellDenyReason::ArgMismatch)
    );
}

#[test]
fn compound_shell_features_rejected() {
    let w = builtin();
    // 管道
    assert_eq!(
        w.deny_reason(&argv(&["Get-Process", "|", "Sort-Object", "CPU"])),
        Some(ShellDenyReason::CompoundShell)
    );
    // 重定向
    assert_eq!(
        w.deny_reason(&argv(&[
            "netsh", "winhttp", "show", "proxy", ">", "out.txt"
        ])),
        Some(ShellDenyReason::CompoundShell)
    );
    // 命令替换
    assert_eq!(
        w.deny_reason(&argv(&["echo", "$(whoami)"])),
        Some(ShellDenyReason::CompoundShell)
    );
    // 白名单程序裸调用（含管道特征）同样被拒
    assert!(!w.is_allowed(&argv(&[
        "Stop-Process",
        "-Id",
        "1",
        "-Force",
        "|",
        "Out-Null"
    ])));
}

#[test]
fn redline_beats_whitelist() {
    // 即使命令命中白名单（Remove-Item 在表内），批量删除红线优先无条件拒
    let w = builtin();
    let e = Enforcer::new(Scope::Fix, &w);
    assert_eq!(
        e.evaluate(&call(
            "shell_exec",
            "Remove-Item C:\\Users\\Jane\\Desktop -Recurse -Force"
        )),
        Verdict::Redline(Redline::BatchDelete)
    );
    assert_eq!(
        e.evaluate(&call("shell_exec", "format c:")),
        Verdict::Redline(Redline::FormatDisk)
    );
}

// ---- 三类草案逐类抽测命中路径（fix 下 write/danger 进审批流） ----

#[test]
fn slow_playbook_hit_path() {
    let w = builtin();
    let e = Enforcer::new(Scope::Fix, &w);
    assert!(w.is_allowed(&argv(&["Stop-Process", "-Id", "1234", "-Force"])));
    assert_eq!(
        e.evaluate(&call("shell_exec", "Stop-Process -Id 1234 -Force")),
        Verdict::NeedApproval(Risk::Write)
    );
}

#[test]
fn popup_playbook_hit_paths() {
    let w = builtin();
    let e = Enforcer::new(Scope::Fix, &w);
    assert!(w.is_allowed(&argv(&["netsh", "winhttp", "show", "proxy"])));
    assert_eq!(
        e.evaluate(&call("shell_exec", "netsh winhttp show proxy")),
        Verdict::NeedApproval(Risk::Write)
    );
    assert!(w.is_allowed(&argv(&[
        "Disable-ScheduledTask",
        "-TaskName",
        "SuspTask",
        "-TaskPath",
        "\\Susp"
    ])));
    assert_eq!(
        e.evaluate(&call(
            "shell_exec",
            "Disable-ScheduledTask -TaskName SuspTask -TaskPath \\Susp"
        )),
        Verdict::NeedApproval(Risk::Write)
    );
}

#[test]
fn c_drive_playbook_hit_paths() {
    let w = builtin();
    let e = Enforcer::new(Scope::Fix, &w);
    assert!(w.is_allowed(&argv(&[
        "Dism.exe",
        "/Online",
        "/Cleanup-Image",
        "/StartComponentCleanup"
    ])));
    assert_eq!(
        e.evaluate(&call(
            "shell_exec",
            "Dism.exe /Online /Cleanup-Image /StartComponentCleanup"
        )),
        Verdict::NeedApproval(Risk::Write)
    );
    assert!(w.is_allowed(&argv(&[
        "Clear-RecycleBin",
        "-Force",
        "-ErrorAction",
        "SilentlyContinue"
    ])));
    // danger 档命令（Remove-Item 系统目录清理）虽命中白名单，但含 -Recurse
    // 与两个操作数（路径 + SilentlyContinue），红线批量删除判定优先拦截。
    assert_eq!(
        e.evaluate(&call(
            "shell_exec",
            "Remove-Item C:\\Windows\\SoftwareDistribution\\Download\\* -Recurse -Force -ErrorAction SilentlyContinue"
        )),
        Verdict::Redline(Redline::BatchDelete)
    );
}
