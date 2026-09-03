//! 逐行游标：解析公共的推进/定位设施（带源码名，供错误溯源）。

use crate::ParseError;

pub(crate) struct Walker<'a> {
    pub(crate) lines: &'a [&'a str],
    pub(crate) idx: usize,
    pub(crate) name: Option<String>,
}

impl<'a> Walker<'a> {
    /// 构造带 1-based 行号与来源名的错误（idx 为 0-based 行下标）。
    pub(crate) fn err(&self, idx: usize, message: impl Into<String>) -> ParseError {
        ParseError {
            source: self.name.clone(),
            line: idx + 1,
            message: message.into(),
        }
    }

    /// 当前行；已到文档末尾返回 None。
    pub(crate) fn cur(&self) -> Option<&'a str> {
        self.lines.get(self.idx).copied()
    }

    /// 越过空行。
    pub(crate) fn skip_blank(&mut self) {
        while self
            .lines
            .get(self.idx)
            .is_some_and(|l| l.trim().is_empty())
        {
            self.idx += 1;
        }
    }

    /// 当前行是否为 `## ` 二级章节标题。
    pub(crate) fn at_heading(&self) -> bool {
        self.lines
            .get(self.idx)
            .is_some_and(|l| l.starts_with("## "))
    }
}
