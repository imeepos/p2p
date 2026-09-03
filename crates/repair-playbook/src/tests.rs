use std::fs;
use std::path::Path;

use super::*;

/// 三类草案 fixture 目录（本 crate 相对仓库根的固定路径）。
const DRAFTS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/playbooks");

fn parse_ok(src: &str) -> Playbook {
    parse(Some("sample.md"), src).unwrap_or_else(|e| panic!("parse error: {e}"))
}

fn parse_err(src: &str) -> String {
    match parse(Some("sample.md"), src) {
        Err(e) => e.to_string(),
        Ok(_) => panic!("expected parse error"),
    }
}

fn doc_with_steps(steps: &str) -> String {
    format!("# Playbook: X\n- 名称: X\n- 问题类别: c\n- 前置条件: a\n## 红线清单\n- r\n{steps}")
}

#[test]
fn sample_parses_fields() {
    let pb = parse_ok(SAMPLE);
    assert_eq!(pb.name, "测试样本");
    assert_eq!(pb.category, "sample-category");
    assert_eq!(pb.runner, None, "推荐 runner 留空应解析为 None");
    assert_eq!(pb.prerequisites.len(), 2);
    assert_eq!(pb.notes.len(), 1);
    assert!(!pb.redlines.is_empty());
    assert_eq!(pb.steps.len(), 2);
    assert_eq!(pb.steps[0].tool, Tool::SysSnapshot);
    assert_eq!(pb.steps[0].title.as_deref(), Some("采集快照"));
    let shell = pb.steps[1].shell.as_ref().unwrap();
    assert_eq!(shell.risk, RiskLevel::Read);
    assert_eq!(shell.step, 2);
    assert_eq!(pb.steps[1].params, None);
    assert_eq!(pb.steps[1].notes.len(), 1);
}

#[test]
fn roundtrip_preserves_semantics() {
    let first = parse_ok(SAMPLE);
    let canonical = first.to_markdown();
    let second = parse_ok(&canonical);
    assert_eq!(
        second.to_markdown(),
        canonical,
        "规范化输出必须可再解析且语义不变"
    );
    assert!(canonical.contains("## 步骤 1"));
}

#[test]
fn error_missing_step_number() {
    let no_number =
        doc_with_steps("## 步骤\n- 说明: d\n- 工具: sys_snapshot\n- 验收: ok\n- 红线: g\n");
    let msg = parse_err(&no_number);
    assert!(msg.contains("步骤号缺失"), "msg = {msg}");
    let bad_number =
        doc_with_steps("## 步骤 修复\n- 说明: d\n- 工具: sys_snapshot\n- 验收: ok\n- 红线: g\n");
    let msg = parse_err(&bad_number);
    assert!(msg.contains("步骤号非法"), "msg = {msg}");
}

#[test]
fn error_step_number_gap() {
    let src = doc_with_steps(
        "## 步骤 1\n- 说明: d\n- 工具: sys_snapshot\n- 验收: ok\n- 红线: g\n
\n## 步骤 3\n- 说明: d\n- 工具: sys_snapshot\n- 验收: ok\n- 红线: g\n",
    );
    let msg = parse_err(&src);
    assert!(msg.contains("步骤号不连续"), "msg = {msg}");
    assert!(msg.contains("期望 2，实际 3"), "msg = {msg}");
}

#[test]
fn error_unknown_tool() {
    let src = doc_with_steps("## 步骤 1\n- 说明: d\n- 工具: sudo_rm_rf\n- 验收: ok\n- 红线: g\n");
    let msg = parse_err(&src);
    assert!(msg.contains("未知工具引用"), "msg = {msg}");
    assert!(msg.contains("sudo_rm_rf"), "msg = {msg}");
}

#[test]
fn error_missing_acceptance() {
    let src = doc_with_steps("## 步骤 1\n- 说明: d\n- 工具: sys_snapshot\n- 红线: g\n");
    let msg = parse_err(&src);
    assert!(msg.contains("缺少验收"), "msg = {msg}");
}

#[test]
fn error_empty_step_redline() {
    let src = doc_with_steps("## 步骤 1\n- 说明: d\n- 工具: sys_snapshot\n- 验收: ok\n- 红线: \n");
    let msg = parse_err(&src);
    assert!(msg.contains("红线不能为空"), "msg = {msg}");
    let missing = doc_with_steps("## 步骤 1\n- 说明: d\n- 工具: sys_snapshot\n- 验收: ok\n");
    let msg = parse_err(&missing);
    assert!(msg.contains("步骤红线不能为空"), "msg = {msg}");
}

#[test]
fn error_empty_overall_redline() {
    let src = "# Playbook: X\n- 名称: X\n- 问题类别: c\n- 前置条件: a\n## 红线清单\n";
    let msg = parse_err(src);
    assert!(msg.contains("整体红线清单不能为空"), "msg = {msg}");
    let empty_item = "# Playbook: X\n- 名称: X\n- 问题类别: c\n- 前置条件: a\n## 红线清单\n- \n";
    let msg = parse_err(empty_item);
    assert!(msg.contains("红线不能为空"), "msg = {msg}");
}

#[test]
fn error_unknown_field() {
    let src = doc_with_steps(
        "## 步骤 1\n- 说明: d\n- 工具: sys_snapshot\n- 验收: ok\n- 红线: g\n- 神秘字段: x\n",
    );
    let msg = parse_err(&src);
    assert!(msg.contains("未知字段"), "msg = {msg}");
    assert!(msg.contains("神秘字段"), "msg = {msg}");
}

#[test]
fn error_carries_line_number() {
    let src = doc_with_steps(
        "## 步骤 1\n- 说明: d\n- 工具: sys_snapshot\n- 验收: ok\n- 红线: g\n- 神秘字段: x\n",
    );
    let msg = parse_err(&src);
    assert!(msg.contains(":12:"), "错误必须带行号，msg = {msg}");
    assert!(msg.contains("sample.md"), "错误必须带来源名，msg = {msg}");
}

#[test]
fn error_missing_command_for_shell() {
    let src = doc_with_steps(
        "## 步骤 1\n- 说明: d\n- 工具: shell_exec\n- 风险档: read\n- 验收: ok\n- 红线: g\n",
    );
    let msg = parse_err(&src);
    assert!(msg.contains("缺少命令"), "msg = {msg}");
}

#[test]
fn error_missing_risk() {
    let src = doc_with_steps(
        "## 步骤 1\n- 说明: d\n- 工具: shell_exec\n- 命令: Get-Date\n- 验收: ok\n- 红线: g\n",
    );
    let msg = parse_err(&src);
    assert!(msg.contains("缺少风险档"), "msg = {msg}");
}

#[test]
fn error_invalid_risk() {
    let src = doc_with_steps("## 步骤 1\n- 说明: d\n- 工具: shell_exec\n- 命令: Get-Date\n- 风险档: extreme\n- 验收: ok\n- 红线: g\n");
    let msg = parse_err(&src);
    assert!(msg.contains("非法风险档"), "msg = {msg}");
}

#[test]
fn error_name_mismatch() {
    let src = "# Playbook: 甲\n- 名称: 乙\n- 问题类别: c\n- 前置条件: a\n## 红线清单\n- r\n## 步骤 1\n- 说明: d\n- 工具: sys_snapshot\n- 验收: ok\n- 红线: g\n";
    let msg = parse_err(src);
    assert!(msg.contains("名称字段与 H1 标题不一致"), "msg = {msg}");
}

#[test]
fn error_missing_params_for_fs_tool() {
    let src = doc_with_steps("## 步骤 1\n- 说明: d\n- 工具: fs_read\n- 验收: ok\n- 红线: g\n");
    let msg = parse_err(&src);
    assert!(msg.contains("缺少参数"), "msg = {msg}");
}

#[test]
fn error_command_without_shell_exec() {
    let src = doc_with_steps(
        "## 步骤 1\n- 说明: d\n- 工具: sys_snapshot\n- 命令: Get-Date\n- 验收: ok\n- 红线: g\n",
    );
    let msg = parse_err(&src);
    assert!(msg.contains("仅用于 shell_exec"), "msg = {msg}");
}

#[test]
fn three_draft_playbooks_parse() {
    for rel in [
        "slow-diagnostics.md",
        "popup-malware-cleanup.md",
        "c-drive-space-cleanup.md",
    ] {
        let path = Path::new(DRAFTS_DIR).join(rel);
        let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let pb = parse(Some(rel), &src).unwrap_or_else(|e| panic!("{rel} 解析失败: {e}"));
        assert!(!pb.category.is_empty(), "{rel}: category 缺失");
        assert!(pb.steps.len() >= 3, "{rel}: 步骤过少");
        for s in &pb.steps {
            assert!(!s.acceptance.is_empty(), "{rel} 步骤 {} 缺验收", s.number);
            assert!(!s.redlines.is_empty(), "{rel} 步骤 {} 缺红线", s.number);
            if s.tool == Tool::ShellExec {
                let sh = s
                    .shell
                    .as_ref()
                    .unwrap_or_else(|| panic!("{rel} 步骤 {} 缺 shell 调用", s.number));
                assert!(!sh.command.is_empty());
            }
        }
        let canonical = pb.to_markdown();
        let reparsed = parse(Some(rel), &canonical)
            .unwrap_or_else(|e| panic!("{rel} 规范化输出不可解析: {e}"));
        assert_eq!(reparsed.to_markdown(), canonical, "{rel} roundtrip 不稳");
    }
}

#[test]
fn load_dir_skips_readme_and_loads_three() {
    let dir = Path::new(DRAFTS_DIR);
    let loaded = load_dir(dir).unwrap_or_else(|e| panic!("load_dir: {e:?}"));
    assert_eq!(loaded.len(), 3, "README 等非 playbook 文件应被跳过");
    let names: Vec<String> = loaded
        .iter()
        .map(|l| l.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().any(|n| n == "slow-diagnostics.md"));
    assert!(names.iter().any(|n| n == "popup-malware-cleanup.md"));
    assert!(names.iter().any(|n| n == "c-drive-space-cleanup.md"));
}

#[test]
fn shell_union_covers_three_categories() {
    let dir = Path::new(DRAFTS_DIR);
    let loaded = load_dir(dir).unwrap();
    let pbs: Vec<&Playbook> = loaded.iter().map(|l| &l.playbook).collect();
    let union = shell_union(&pbs);
    assert!(
        union.iter().any(|c| c.contains("Get-Process |")),
        "卡慢类命令缺失"
    );
    assert!(
        union.iter().any(|c| c.contains("netsh winhttp show proxy")),
        "弹窗类命令缺失"
    );
    assert!(
        union.iter().any(|c| c.contains("Clear-RecycleBin")),
        "C 盘类命令缺失"
    );
    let mut sorted = union.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), union.len(), "并集必须去重");
    let risks: Vec<RiskLevel> = pbs
        .iter()
        .flat_map(|p| p.shell_commands())
        .map(|s| s.risk)
        .collect();
    assert!(risks.contains(&RiskLevel::Read), "并集缺少 read 档");
    assert!(risks.contains(&RiskLevel::Write), "并集缺少 write 档");
    assert!(risks.contains(&RiskLevel::Danger), "并集缺少 danger 档");
}

#[test]
fn shell_commands_step_order_and_argv0() {
    let pb = parse_ok(SAMPLE);
    let cmds = pb.shell_commands();
    assert_eq!(cmds.len(), 1);
    let inv = cmds[0];
    assert_eq!(inv.step, 2);
    assert_eq!(inv.argv0(), "Get-Process");
    let cmd = ShellInvocation {
        step: 9,
        command: "cmd /c dir C:\\Windows".into(),
        risk: RiskLevel::Read,
        line: 1,
    };
    assert_eq!(cmd.argv0(), "cmd");
}

#[test]
fn error_load_dir_collects_all_failures() {
    let dir = std::env::temp_dir().join(format!("rs-playbook-bad-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("one.md"), "# Playbook: 甲\n- 名称: 甲\n- 问题类别: c\n- 前置条件: a\n## 红线清单\n- r\n## 步骤 2\n- 说明: d\n- 工具: sys_snapshot\n- 验收: ok\n- 红线: g\n").unwrap();
    fs::write(dir.join("two.md"), "# Playbook: 乙\n- 名称: 乙\n- 问题类别: c\n- 前置条件: a\n## 红线清单\n- r\n## 步骤 1\n- 说明: d\n- 工具: weird_tool\n- 验收: ok\n- 红线: g\n").unwrap();
    let errs = load_dir(&dir).unwrap_err();
    assert_eq!(errs.len(), 2, "全部失败必须收集，不静默");
    assert!(errs.iter().any(|e| e.message.contains("步骤号不连续")));
    assert!(errs.iter().any(|e| e.message.contains("未知工具引用")));
    fs::remove_dir_all(&dir).unwrap();
}
