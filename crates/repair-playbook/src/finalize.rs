//! 装配 playbook：前置字段 + 红线 + 步骤的交叉校验与组装。

use crate::front::FrontFields;
use crate::walker::Walker;
use crate::{ParseError, Playbook, Step};

pub(crate) fn finalize(
    w: &Walker<'_>,
    h1: &str,
    f: FrontFields,
    redlines: Vec<String>,
    steps: Vec<Step>,
) -> Result<Playbook, ParseError> {
    let name = match &f.name {
        Some((idx, n)) => {
            if n != h1 {
                return Err(w.err(*idx, format!("名称字段与 H1 标题不一致: {n}")));
            }
            n.clone()
        }
        None => return Err(w.err(f.end, "缺少名称字段")),
    };
    let category = match f.category {
        Some((_, c)) => c,
        None => return Err(w.err(f.end, "缺少问题类别字段")),
    };
    if f.prerequisites.is_empty() {
        return Err(w.err(f.end, "前置条件不能为空"));
    }
    if redlines.is_empty() {
        return Err(w.err(f.end, "整体红线清单不能为空"));
    }
    if steps.is_empty() {
        return Err(w.err(f.end, "至少需要一个步骤"));
    }
    Ok(Playbook {
        name,
        category,
        runner: f.runner,
        prerequisites: f.prerequisites,
        notes: f.notes,
        redlines,
        steps,
    })
}
