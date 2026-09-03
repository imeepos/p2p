//! 二级章节解析：`## 红线清单` 与 `## 步骤 <N>` 步骤体及字段校验。

use crate::walker::Walker;
use crate::{ParseError, RiskLevel, ShellInvocation, Step, Tool};

pub(crate) fn parse_sections(w: &mut Walker<'_>) -> Result<(Vec<String>, Vec<Step>), ParseError> {
    let mut redlines = Vec::new();
    let mut steps = Vec::new();
    let mut expected: u32 = 1;
    loop {
        w.skip_blank();
        let line = match w.cur() {
            Some(l) => l,
            None => break,
        };
        let idx = w.idx;
        let heading = match line.strip_prefix("## ") {
            Some(h) => h,
            None => {
                return Err(w.err(
                    idx,
                    format!("意外的裸行（应为 ## 章节标题、- 字段行或空行）: {line}"),
                ));
            }
        };
        if heading == "红线清单" {
            if !redlines.is_empty() {
                return Err(w.err(idx, "重复的红线清单章节"));
            }
            redlines = parse_bullets(w)?;
        } else if let Some(rest) = heading.strip_prefix("步骤 ") {
            steps.push(parse_step(w, rest, expected)?);
            expected += 1;
        } else if heading.starts_with("步骤") {
            return Err(w.err(idx, "步骤号缺失（格式: ## 步骤 <N> [标题]）"));
        } else {
            return Err(w.err(idx, format!("未知章节: ## {heading}")));
        }
    }
    Ok((redlines, steps))
}

fn parse_bullets(w: &mut Walker<'_>) -> Result<Vec<String>, ParseError> {
    w.idx += 1; // 越过 ## 红线清单
    let mut items = Vec::new();
    loop {
        w.skip_blank();
        if w.at_heading() || w.cur().is_none() {
            break;
        }
        let idx = w.idx;
        let item = match w.cur().unwrap_or("").strip_prefix("- ") {
            Some(i) => i,
            None => return Err(w.err(idx, "红线清单只允许 - 列表项")),
        };
        let item = item.trim();
        if item.is_empty() {
            return Err(w.err(idx, "红线不能为空"));
        }
        items.push(item.to_string());
        w.idx += 1;
    }
    Ok(items)
}

fn parse_step(w: &mut Walker<'_>, rest: &str, expected: u32) -> Result<Step, ParseError> {
    let head_idx = w.idx;
    let (num_str, title) = split_number_title(rest);
    let Ok(number) = num_str.parse::<u32>() else {
        return Err(w.err(head_idx, format!("步骤号非法: {num_str}（应为正整数）")));
    };
    if number != expected {
        return Err(w.err(
            head_idx,
            format!("步骤号不连续：期望 {expected}，实际 {number}"),
        ));
    }
    let mut fields = StepFields::default();
    w.idx += 1; // 越过步骤标题
    loop {
        w.skip_blank();
        if w.at_heading() || w.cur().is_none() {
            break;
        }
        let idx = w.idx;
        let line = w.cur().unwrap_or("");
        if !line.starts_with("- ") {
            return Err(w.err(idx, "步骤体只允许 - 键: 值 字段行"));
        }
        let kv = &line[2..];
        let (key, value) = match kv.split_once(": ") {
            Some(p) => p,
            None => return Err(w.err(idx, "字段缺少冒号分隔")),
        };
        fields.apply(key.trim(), value.trim(), idx, w)?;
        w.idx += 1;
    }
    finalize_step(fields, number, title, head_idx, w)
}

fn split_number_title(rest: &str) -> (&str, Option<String>) {
    match rest.split_once(char::is_whitespace) {
        Some((n, t)) => {
            let t = t.trim();
            (
                n,
                if t.is_empty() {
                    None
                } else {
                    Some(t.to_string())
                },
            )
        }
        None => (rest, None),
    }
}

#[derive(Default)]
struct StepFields {
    description: Option<(usize, String)>,
    tool: Option<(usize, String)>,
    params: Option<(usize, String)>,
    command: Option<(usize, String)>,
    risk: Option<(usize, String)>,
    acceptance: Option<(usize, String)>,
    redlines: Vec<String>,
    notes: Vec<String>,
}

impl StepFields {
    fn apply(
        &mut self,
        key: &str,
        value: &str,
        idx: usize,
        w: &Walker<'_>,
    ) -> Result<(), ParseError> {
        match key {
            "说明" => set(&mut self.description, value, idx, w, "说明不能为空"),
            "工具" => set(&mut self.tool, value, idx, w, "工具不能为空"),
            "参数" => set(&mut self.params, value, idx, w, "参数不能为空"),
            "命令" => set(&mut self.command, value, idx, w, "命令不能为空"),
            "风险档" => set(&mut self.risk, value, idx, w, "风险档不能为空"),
            "验收" => set(&mut self.acceptance, value, idx, w, "验收不能为空"),
            "红线" => {
                if value.is_empty() {
                    return Err(w.err(idx, "红线不能为空"));
                }
                self.redlines.push(value.to_string());
                Ok(())
            }
            "备注" => {
                if value.is_empty() {
                    return Err(w.err(idx, "备注不能为空"));
                }
                self.notes.push(value.to_string());
                Ok(())
            }
            _ => Err(w.err(idx, format!("未知字段: {key}"))),
        }
    }
}

fn set(
    slot: &mut Option<(usize, String)>,
    value: &str,
    idx: usize,
    w: &Walker<'_>,
    empty_msg: &str,
) -> Result<(), ParseError> {
    if slot.is_some() {
        return Err(w.err(idx, "重复字段"));
    }
    if value.is_empty() {
        return Err(w.err(idx, empty_msg));
    }
    *slot = Some((idx, value.to_string()));
    Ok(())
}

fn finalize_step(
    f: StepFields,
    number: u32,
    title: Option<String>,
    head_idx: usize,
    w: &Walker<'_>,
) -> Result<Step, ParseError> {
    let description = required(&f.description, head_idx, "缺少说明", w)?;
    let tool = required(&f.tool, head_idx, "缺少工具", w)?;
    let step_tool = match Tool::from_name(&tool) {
        Some(t) => t,
        None => return Err(w.err(head_idx, format!("未知工具引用: {tool}"))),
    };
    if f.redlines.is_empty() {
        return Err(w.err(head_idx, "步骤红线不能为空"));
    }
    if step_tool.needs_params() && f.params.is_none() {
        return Err(w.err(head_idx, "缺少参数（fs_* 工具必须给出参数要点）"));
    }
    if step_tool != Tool::ShellExec && (f.command.is_some() || f.risk.is_some()) {
        return Err(w.err(head_idx, "命令/风险档字段仅用于 shell_exec 步骤"));
    }
    let acceptance = required(&f.acceptance, head_idx, "缺少验收", w)?;
    let shell = match step_tool {
        Tool::ShellExec => Some(ShellInvocation {
            step: number,
            command: required(
                &f.command,
                head_idx,
                "缺少命令（shell_exec 步骤必须给出 shell 命令）",
                w,
            )?,
            risk: parse_risk(&f.risk, head_idx, w)?,
            line: head_idx + 1,
        }),
        _ => None,
    };
    Ok(Step {
        number,
        title,
        description,
        tool: step_tool,
        params: f.params.map(|(_, v)| v),
        shell,
        acceptance,
        redlines: f.redlines,
        notes: f.notes,
        line: head_idx + 1,
    })
}

fn required(
    slot: &Option<(usize, String)>,
    fallback: usize,
    msg: &str,
    w: &Walker<'_>,
) -> Result<String, ParseError> {
    match slot {
        Some((_, v)) => Ok(v.clone()),
        None => Err(w.err(fallback, msg)),
    }
}

fn parse_risk(
    slot: &Option<(usize, String)>,
    fallback: usize,
    w: &Walker<'_>,
) -> Result<RiskLevel, ParseError> {
    let (idx, raw) = match slot {
        Some(t) => t,
        None => {
            return Err(w.err(
                fallback,
                "缺少风险档（shell_exec 步骤必须标注 read/write/danger）",
            ))
        }
    };
    match RiskLevel::from_name(raw) {
        Some(level) => Ok(level),
        None => Err(w.err(*idx, format!("非法风险档: {raw}（read/write/danger）"))),
    }
}
