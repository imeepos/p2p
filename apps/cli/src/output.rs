//! 输出层：默认人读文本，--json 时输出结构化 JSON（两形态共用同一事实源）。

use serde::Serialize;

use crate::error::CliError;

/// 渲染输出：json=true 给出可解析 JSON，否则给人读文本。
pub fn render<T: Serialize>(json: bool, value: &T, text: &str) -> Result<String, CliError> {
    if json {
        serde_json::to_string_pretty(value)
            .map_err(|e| CliError::Runtime(format!("JSON 序列化失败: {e}")))
    } else {
        Ok(text.to_string())
    }
}

/// 渲染并打印到 stdout。
pub fn emit<T: Serialize>(json: bool, value: &T, text: &str) -> Result<(), CliError> {
    println!("{}", render(json, value, text)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_mode_output_is_parseable() {
        let value = json!({ "running": false, "state": "not_running" });
        let rendered = render(true, &value, "ignored").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(parsed["running"], json!(false));
    }

    #[test]
    fn text_mode_output_is_verbatim() {
        let value = json!({ "running": false });
        assert_eq!(render(false, &value, "节点未运行").unwrap(), "节点未运行");
    }
}
