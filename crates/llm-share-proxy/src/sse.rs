//! SSE 事件切分与 usage 提取（§4 步骤 5/6）：转发事件原文，usage 只认 OpenAI 的
//! prompt_tokens/completion_tokens；无 usage 时按字节估算（断流计费，收据 estimated=true）。

use llm_share_ledger::Usage;
use serde_json::Value;

/// 累积上游字节并按空行切分完整 SSE 事件；容忍 \r\n 行尾。
#[derive(Default)]
pub struct SseSplitter {
    buf: Vec<u8>,
}

impl SseSplitter {
    /// 喂入新字节，返回其中已完整的事件（原文，去首尾空白行）。
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(consumed) = Self::boundary(&self.buf) {
            let raw: Vec<u8> = self.buf.drain(..consumed).collect();
            Self::push_event(&raw, &mut events);
        }
        events
    }

    /// 流终止时冲刷残余半截事件：断流场景已实际消费，必须计入估算口径。
    pub fn finish(&mut self) -> Option<String> {
        if self.buf.is_empty() {
            return None;
        }
        let raw = std::mem::take(&mut self.buf);
        let text = String::from_utf8_lossy(&raw).trim().to_string();
        (!text.is_empty()).then_some(text)
    }

    /// 事件分隔 = \n 后跟可选 \r 再 \n（覆盖 \n\n 与 \r\n\r\n），返回消耗长度。
    fn boundary(buf: &[u8]) -> Option<usize> {
        for (i, b) in buf.iter().enumerate() {
            if *b != b'\n' {
                continue;
            }
            let mut j = i + 1;
            if buf.get(j) == Some(&b'\r') {
                j += 1;
            }
            if buf.get(j) == Some(&b'\n') {
                return Some(j + 1);
            }
        }
        None
    }

    fn push_event(raw: &[u8], out: &mut Vec<String>) {
        let text = String::from_utf8_lossy(raw);
        let trimmed = text.trim_matches(|c| c == '\r' || c == '\n');
        if !trimmed.is_empty() {
            out.push(trimmed.to_string());
        }
    }
}

/// 从单个 SSE 事件提取 usage（流末 chunk 携带）；非 JSON data 行与 [DONE] 忽略。
pub fn extract_usage(event: &str) -> Option<Usage> {
    let mut found = None;
    for line in event.lines() {
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        let Some(usage) = v.get("usage") else {
            continue;
        };
        let Some(input) = usage.get("prompt_tokens").and_then(Value::as_u64) else {
            continue;
        };
        let Some(output) = usage.get("completion_tokens").and_then(Value::as_u64) else {
            continue;
        };
        found = Some(Usage { input, output });
    }
    found
}

/// token 估算：约 4 字节/token 向上取整；输入估算与断流估算共用同一口径（§4）。
pub fn estimate_tokens(bytes: usize) -> u64 {
    (bytes as u64).div_ceil(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitter_handles_crlf_and_batched_events() {
        let mut s = SseSplitter::default();
        let events = s.feed(b"data: a\r\n\r\ndata: b\n\ndata: c");
        assert_eq!(events, ["data: a", "data: b"]);
        assert_eq!(s.finish().as_deref(), Some("data: c"));
        assert_eq!(s.finish(), None);
    }

    #[test]
    fn usage_extracted_from_final_chunk_only() {
        assert_eq!(
            extract_usage(
                "data: {\"choices\":[]}

"
            ),
            None
        );
        assert_eq!(extract_usage("data: [DONE]"), None);
        let event = "data: {\"id\":1,\"usage\":{\"prompt_tokens\":1234,\"completion_tokens\":567}}";
        let usage = extract_usage(event).expect("usage");
        assert_eq!((usage.input, usage.output), (1234, 567));
    }

    #[test]
    fn estimate_rounds_up() {
        assert_eq!(estimate_tokens(0), 0);
        assert_eq!(estimate_tokens(1), 1);
        assert_eq!(estimate_tokens(9), 3);
    }
}
