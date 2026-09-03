//! 输出门禁：单次调用结果上限与安全截断。
//!
//! 语义（remote-support-plan.md §3.5）：单结果 ≤ [MAX_OUTPUT_BYTES]，超限截断
//! 并置 truncated=true；截断必须落在 UTF-8 字符边界（[str::floor_char_boundary]），
//! 杜绝非法索引 panic。

use crate::ToolResult;

/// 单次调用结果上限（256 KiB）。
pub const MAX_OUTPUT_BYTES: usize = 256 * 1024;

/// 对工具结果应用输出门禁（幂等：已截断的短文本不再变化）。
/// 宿主在每次 tools/call 后统一应用，误报告大小的工具也无法突破上限。
pub fn apply_output_gate(mut result: ToolResult) -> ToolResult {
    if result.text.len() > MAX_OUTPUT_BYTES {
        let idx = result.text.floor_char_boundary(MAX_OUTPUT_BYTES);
        result.text.truncate(idx);
        result.truncated = true;
    }
    result
}

/// 文本摘要：超限截断到 max_bytes 内并追加省略标记（审计/日志用）。
pub fn head(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let idx = text.floor_char_boundary(max_bytes.saturating_sub(3));
    format!("{}...", &text[..idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_limit_untouched() {
        let result = apply_output_gate(ToolResult {
            text: "small".into(),
            truncated: false,
        });
        assert_eq!(result.text, "small");
        assert!(!result.truncated);
    }

    #[test]
    fn over_limit_truncated_and_flagged() {
        let text = "a".repeat(MAX_OUTPUT_BYTES + 1);
        let result = apply_output_gate(ToolResult {
            text,
            truncated: false,
        });
        assert!(result.truncated);
        assert_eq!(result.text.len(), MAX_OUTPUT_BYTES);
    }

    #[test]
    fn truncation_keeps_char_boundary() {
        let mut text = "a".repeat(MAX_OUTPUT_BYTES - 1);
        text.push_str("界界");
        let result = apply_output_gate(ToolResult {
            text,
            truncated: false,
        });
        assert!(result.truncated);
        assert!(result.text.is_char_boundary(result.text.len()));
        assert!(result.text.len() <= MAX_OUTPUT_BYTES);
    }

    #[test]
    fn head_shortens_with_marker() {
        let long = "x".repeat(100);
        assert_eq!(head(&long, 10), "xxxxxxx...");
        assert!(head(&long, 10).len() <= 10);
        assert_eq!(head(&long, 1000), long);
    }

    #[test]
    fn head_keeps_char_boundary() {
        let s = "价".repeat(50);
        let out = head(&s, 20);
        assert!(out.is_char_boundary(out.len()));
    }
}
