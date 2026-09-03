//! 解析入口与目录装载：受控 markdown -> [Playbook]（格式规范见 crate 文档）。
//!
//! 一切校验失败返回带行号的 [ParseError]，禁止静默忽略；目录装载 [load_dir] 对
//! 失败文件逐个发出 tracing::warn!，非 playbook 文件跳过并留 debug 日志。

use std::path::Path;

use crate::walker::Walker;
use crate::{LoadedPlaybook, ParseError, Playbook};

/// 解析单个 playbook 文本。
///
/// `name` 仅用于错误信息溯源（通常传文件名）；不带来源时传 None。
pub fn parse(name: Option<&str>, source: &str) -> Result<Playbook, ParseError> {
    let lines: Vec<&str> = source.lines().collect();
    let mut w = Walker {
        lines: &lines,
        idx: 0,
        name: name.map(str::to_string),
    };
    let h1 = parse_h1(&mut w)?;
    let fields = crate::front::parse_front_matter(&mut w)?;
    let (redlines, steps) = crate::steps::parse_sections(&mut w)?;
    crate::finalize::finalize(&w, &h1, fields, redlines, steps)
}

/// 解析目录下全部 `*.md`：首个非空行为 `# Playbook: ` 的文件视为 playbook，
/// 其余跳过（留 debug 日志）；任一 playbook 失败即整体 Err 并列出全部错因。
pub fn load_dir(dir: &Path) -> Result<Vec<LoadedPlaybook>, Vec<ParseError>> {
    let mut markdown = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(es) => es,
        Err(e) => return Err(file_error(dir, &format!("读取目录失败: {e}"))),
    };
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => return Err(file_error(dir, &format!("读取目录项失败: {e}"))),
        };
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "md") {
            markdown.push(path);
        }
    }
    markdown.sort();
    let mut loaded = Vec::new();
    let mut errors = Vec::new();
    for path in markdown {
        load_one(&path, &mut loaded, &mut errors);
    }
    if errors.is_empty() {
        Ok(loaded)
    } else {
        Err(errors)
    }
}

fn load_one(path: &Path, loaded: &mut Vec<LoadedPlaybook>, errors: &mut Vec<ParseError>) {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("load_dir: 读取 {} 失败: {e}", path.display());
            errors.push(ParseError {
                source: name,
                line: 0,
                message: format!("读取文件失败: {e}"),
            });
            return;
        }
    };
    let first = src.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    if !first.starts_with("# Playbook: ") {
        tracing::debug!("load_dir: 跳过非 playbook 文件 {}", path.display());
        return;
    }
    match parse(name.as_deref(), &src) {
        Ok(playbook) => loaded.push(LoadedPlaybook {
            path: path.to_path_buf(),
            playbook,
        }),
        Err(e) => {
            tracing::warn!("load_dir: {} 解析失败: {}", path.display(), e);
            errors.push(e);
        }
    }
}

fn file_error(dir: &Path, message: &str) -> Vec<ParseError> {
    vec![ParseError {
        source: Some(dir.display().to_string()),
        line: 0,
        message: message.to_string(),
    }]
}

fn parse_h1(w: &mut Walker<'_>) -> Result<String, ParseError> {
    w.skip_blank();
    let idx = w.idx;
    let line = match w.cur() {
        Some(l) => l,
        None => return Err(w.err(idx, "空文档：缺少 # Playbook: 标题")),
    };
    let rest = match line.strip_prefix("# Playbook: ") {
        Some(r) => r,
        None => return Err(w.err(idx, "首行必须是 # Playbook: <名称>")),
    };
    let name = rest.trim();
    if name.is_empty() {
        return Err(w.err(idx, "H1 名称不能为空"));
    }
    w.idx += 1;
    Ok(name.to_string())
}
