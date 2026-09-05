//! 输出层：默认人读文本，--json 时输出结构化 JSON（两形态共用同一事实源）。
//! 写后显式 flush：start 类命令在同进程内拉起守护进程，若输出滞留缓冲
//! 而进程随后异常退出，重定向文件会拿到 0 字节（F9）。

use std::io::Write;

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

/// 渲染并打印到 stdout，写后显式 flush（失败转运行错误，不静默）。
pub fn emit<T: Serialize>(json: bool, value: &T, text: &str) -> Result<(), CliError> {
    write_line(&mut std::io::stdout().lock(), &render(json, value, text)?)
}

/// 向任意 writer 写一行并冲刷（emit 的可测内核：回归覆盖重定向文件形态）。
pub fn write_line<W: Write>(w: &mut W, payload: &str) -> Result<(), CliError> {
    w.write_all(payload.as_bytes())
        .and_then(|_| w.write_all(b"\n"))
        .and_then(|_| w.flush())
        .map_err(|e| CliError::Runtime(format!("stdout 写出失败: {e}")))
}

/// 拉起子进程前冲刷父进程 stdio（spawn 前的缓冲纪律，F9）。
pub fn flush_stdio() {
    if let Err(e) = std::io::stdout().flush() {
        eprintln!("p2pctl: stdout flush 失败: {e}");
    }
    let _ = std::io::stderr().flush();
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

    #[test]
    fn emit_to_file_is_non_empty_and_parseable() {
        // F9 回归：重定向到文件时内容必须完整落盘（写后 flush，非进程退出兜底）。
        let dir = std::env::temp_dir().join(format!("p2pctl-out-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("start.json");
        let value = json!({ "running": true, "pid": 7 });
        {
            let mut f = std::fs::File::create(&path).unwrap();
            write_line(&mut f, &render(true, &value, "ignored").unwrap()).unwrap();
        }
        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.is_empty(), "重定向文件非空");
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed["pid"], json!(7));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
