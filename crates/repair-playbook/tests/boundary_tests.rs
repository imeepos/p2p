use std::fs;
use std::path::Path;

use repair_playbook::{
    load_dir, parse, shell_union, Playbook, RiskLevel, ShellInvocation, Step, Tool,
};

const VALID: &str = "# Playbook: 边界样本\n- 名称: 边界样本\n- 问题类别: boundary\n- 前置条件: Windows 10/11\n## 红线清单\n- 不格式化磁盘\n## 步骤 1 查询\n- 说明: 读取状态\n- 工具: sys_snapshot\n- 验收: 返回成功\n- 红线: 只读\n";

fn error_text(source: &str) -> String {
    parse(Some("boundary.md"), source)
        .expect_err("input must fail")
        .to_string()
}

fn shell_playbook(command: &str) -> Playbook {
    Playbook {
        name: "shell".into(),
        category: "boundary".into(),
        runner: None,
        prerequisites: vec!["Windows".into()],
        notes: Vec::new(),
        redlines: vec!["禁止危险操作".into()],
        steps: vec![Step {
            number: 1,
            title: None,
            description: "查询".into(),
            tool: Tool::ShellExec,
            params: None,
            shell: Some(ShellInvocation {
                step: 1,
                command: command.into(),
                risk: RiskLevel::Read,
                line: 1,
            }),
            acceptance: "输出存在".into(),
            redlines: vec!["只读".into()],
            notes: Vec::new(),
            line: 1,
        }],
    }
}

#[test]
fn missing_front_matter_fields_have_positioned_errors() {
    assert!(error_text(&VALID.replacen("- 名称: 边界样本\n", "", 1)).contains("缺少名称字段"));
    assert!(
        error_text(&VALID.replacen("- 问题类别: boundary\n", "", 1)).contains("缺少问题类别字段")
    );
    assert!(
        error_text(&VALID.replacen("- 前置条件: Windows 10/11\n", "", 1))
            .contains("前置条件不能为空")
    );
}

#[test]
fn duplicate_step_number_is_rejected() {
    let source = VALID.replace("## 步骤 1 查询\n", "## 步骤 1 查询\n- 说明: 读取状态\n- 工具: sys_snapshot\n- 验收: 返回成功\n- 红线: 只读\n\n## 步骤 1 再查\n");
    let text = error_text(&source);
    assert!(text.contains("步骤号不连续") && text.contains("boundary.md:"));
}

#[test]
fn shell_step_without_command_is_rejected() {
    let text = error_text(&VALID.replace("工具: sys_snapshot", "工具: shell_exec"));
    assert!(text.contains("缺少命令") && text.contains("boundary.md:"));
}

#[test]
fn empty_redline_and_empty_step_list_are_rejected() {
    assert!(error_text(&VALID.replace("- 不格式化磁盘\n", "")).contains("整体红线清单不能为空"));
    let no_steps = VALID.split("## 步骤 1 查询").next().expect("front matter");
    assert!(error_text(no_steps).contains("至少需要一个步骤"));
}

#[test]
fn crlf_lines_are_accepted() {
    let parsed = parse(Some("crlf.md"), &VALID.replace("\n", "\r\n")).expect("CRLF");
    assert_eq!(parsed.steps.len(), 1);
}

#[test]
fn bom_is_reported_with_a_clear_position() {
    let text = error_text(&format!("\u{feff}{VALID}"));
    assert!(text.contains("boundary.md:1:") && (text.contains("首行必须") || text.contains("H1")));
}

#[test]
fn long_single_line_value_is_preserved() {
    let value = "x".repeat(16 * 1024);
    let source = VALID.replace("- 说明: 读取状态", &format!("- 说明: {value}"));
    let parsed = parse(None, &source).expect("long field");
    assert_eq!(parsed.steps[0].description.len(), value.len());
}

#[test]
fn special_characters_survive_roundtrip() {
    let source = VALID.replace(
        "- 说明: 读取状态",
        "- 说明: 引号 \"双\" 与反引号 `code` 以及竖线 | 保持",
    );
    let first = parse(None, &source).expect("special chars");
    let canonical = first.to_markdown();
    let second = parse(None, &canonical).expect("canonical");
    assert_eq!(second.steps[0].description, first.steps[0].description);
    assert!(canonical.contains("`code`") && canonical.contains("|"));
}

#[test]
fn shell_union_handles_empty_and_shell_free_inputs() {
    let empty: Vec<&Playbook> = Vec::new();
    assert!(shell_union(&empty).is_empty());
    let no_shell = parse(None, VALID).expect("shell-free input");
    assert!(shell_union(&[&no_shell]).is_empty());
}

#[test]
fn shell_union_deduplicates_same_command_across_playbooks() {
    let one = shell_playbook("Get-Date");
    let two = shell_playbook("Get-Date");
    let three = shell_playbook("Get-Process");
    assert_eq!(
        shell_union(&[&one, &two, &three]),
        vec!["Get-Date", "Get-Process"]
    );
}

#[test]
fn load_dir_error_order_is_lexicographically_stable() {
    let dir = std::env::temp_dir().join(format!("repair-playbook-order-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    fs::write(dir.join("z-last.md"), "# Playbook: Z\n- 名称: Z\n").expect("write z");
    fs::write(dir.join("a-first.md"), "# Playbook: A\n- 名称: A\n").expect("write a");
    let errors = load_dir(&dir).expect_err("malformed files");
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].source.as_deref(), Some("a-first.md"));
    assert_eq!(errors[1].source.as_deref(), Some("z-last.md"));
    fs::remove_dir_all(&dir).expect("remove temp dir");
}

#[test]
fn shell_invocation_argv0_handles_empty_command() {
    let empty = ShellInvocation {
        step: 1,
        command: String::new(),
        risk: RiskLevel::Read,
        line: 1,
    };
    assert_eq!(empty.argv0(), "");
    assert_eq!(
        shell_playbook("cmd /c dir").shell_commands()[0].argv0(),
        "cmd"
    );
}

#[test]
fn three_drafts_remain_export_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/playbooks");
    let loaded = load_dir(&root).expect("draft playbooks");
    assert_eq!(loaded.len(), 3);
    let refs: Vec<&Playbook> = loaded.iter().map(|item| &item.playbook).collect();
    let commands = shell_union(&refs);
    assert!(commands
        .iter()
        .any(|command| command.contains("Get-Process")));
    assert!(commands
        .iter()
        .any(|command| command.contains("Clear-RecycleBin")));
    assert!(commands
        .iter()
        .any(|command| command.contains("netsh winhttp show proxy")));
    // 消融抽测口径：临时改坏任一草案命令时，本断言必须失败；不提交被改坏的 fixture。
}
