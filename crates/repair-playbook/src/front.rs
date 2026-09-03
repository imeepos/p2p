//! 前置字段区解析：H1 与 `## ` 章节之间的 `- 键: 值` 字段块。

use crate::walker::Walker;
use crate::ParseError;

pub struct FrontFields {
    pub name: Option<(usize, String)>,
    pub category: Option<(usize, String)>,
    pub runner: Option<String>,
    pub prerequisites: Vec<String>,
    pub notes: Vec<String>,
    pub end: usize,
}

pub(crate) fn parse_front_matter(w: &mut Walker<'_>) -> Result<FrontFields, ParseError> {
    let mut f = FrontFields {
        name: None,
        category: None,
        runner: None,
        prerequisites: Vec::new(),
        notes: Vec::new(),
        end: 0,
    };
    loop {
        w.skip_blank();
        if w.at_heading() || w.cur().is_none() {
            f.end = w.idx;
            break;
        }
        let idx = w.idx;
        let line = w.cur().unwrap_or("");
        if !line.starts_with("- ") {
            return Err(w.err(idx, "前置区只允许 - 键: 值 字段行"));
        }
        // 推荐 runner/前置条件 允许留空：「- 键:」无冒号空格的裸写字法等价于空值。
        let (key, value) = match split_kv(&line[2..], idx, w) {
            Ok(pair) => pair,
            Err(_) => {
                let bare = line[2..].trim_end_matches(':').trim();
                if bare == "推荐 runner" || bare == "前置条件" {
                    (bare, "")
                } else {
                    return Err(w.err(idx, "字段缺少冒号分隔"));
                }
            }
        };
        match key {
            "名称" => {
                if f.name.is_some() {
                    return Err(w.err(idx, "重复字段: 名称"));
                }
                if value.is_empty() {
                    return Err(w.err(idx, "名称不能为空"));
                }
                f.name = Some((idx, value.to_string()));
            }
            "问题类别" => {
                if f.category.is_some() {
                    return Err(w.err(idx, "重复字段: 问题类别"));
                }
                if value.is_empty() {
                    return Err(w.err(idx, "问题类别不能为空"));
                }
                f.category = Some((idx, value.to_string()));
            }
            "推荐 runner" => {
                if f.runner.is_some() {
                    return Err(w.err(idx, "重复字段: 推荐 runner"));
                }
                f.runner = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            "前置条件" => append_prerequisite(w, &mut f, idx, value)?,
            "备注" => {
                if value.is_empty() {
                    return Err(w.err(idx, "备注不能为空"));
                }
                f.notes.push(value.to_string());
            }
            _ => return Err(w.err(idx, format!("未知字段: {key}"))),
        }
        w.idx += 1;
    }
    Ok(f)
}

fn split_kv<'a>(kv: &'a str, idx: usize, w: &Walker<'_>) -> Result<(&'a str, &'a str), ParseError> {
    match kv.split_once(": ") {
        Some((key, value)) => Ok((key.trim(), value.trim())),
        None => Err(w.err(idx, "字段缺少冒号分隔")),
    }
}

fn append_prerequisite(
    w: &mut Walker<'_>,
    f: &mut FrontFields,
    idx: usize,
    value: &str,
) -> Result<(), ParseError> {
    if value.is_empty() {
        let mut count = 0;
        while let Some(next) = w.lines.get(w.idx + 1) {
            let item = match next.strip_prefix("  - ") {
                Some(i) => i,
                None => break,
            };
            let item = item.trim();
            if item.is_empty() {
                return Err(w.err(w.idx + 1, "前置条件项不能为空"));
            }
            f.prerequisites.push(item.to_string());
            w.idx += 1;
            count += 1;
        }
        if count == 0 {
            return Err(w.err(idx, "前置条件不能为空"));
        }
    } else {
        f.prerequisites.push(value.to_string());
    }
    Ok(())
}
