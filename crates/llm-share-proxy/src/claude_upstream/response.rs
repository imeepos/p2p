//! Claude SSE → OpenAI SSE 翻译状态机（§5.2）：复用 [crate::sse::SseSplitter]
//! 跨 reqwest 字节块切分事件，缓存 message_start input_tokens 供 message_delta
//! 合成 OpenAI usage chunk；只输出 OpenAI SSE 行，不透传 Claude 原生 event。

use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use serde_json::{json, Value};

use crate::sse::SseSplitter;
use crate::upstream::{SseByteStream, UpstreamFailure};

/// 翻译状态机；流内畸形 JSON / 上游 error 事件 / 缺 message_stop 均显式失败。
pub struct ClaudeSseTranslator {
    inner: SseByteStream,
    splitter: SseSplitter,
    input_tokens: Option<u64>,
    saw_message_stop: bool,
    pending: VecDeque<Vec<u8>>,
    failure: Option<UpstreamFailure>,
    done: bool,
}

impl ClaudeSseTranslator {
    pub fn new(inner: SseByteStream) -> Self {
        Self {
            inner,
            splitter: SseSplitter::default(),
            input_tokens: None,
            saw_message_stop: false,
            pending: VecDeque::new(),
            failure: None,
            done: false,
        }
    }

    fn queue(&mut self, value: &Value) {
        let line = format!("data: {value}\n\n");
        self.pending.push_back(line.into_bytes());
    }

    fn handle_event(&mut self, event: &str) {
        let Some(data) = event
            .lines()
            .find_map(|l| l.strip_prefix("data:").map(str::trim))
        else {
            return; // 纯注释/心跳事件
        };
        if data == "[DONE]" {
            return;
        }
        let v: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(e) => {
                self.failure = Some(UpstreamFailure::Broken(format!(
                    "claude event not json: {e}"
                )));
                return;
            }
        };
        let Some(typ) = v.get("type").and_then(Value::as_str) else {
            tracing::warn!("claude event without type ignored");
            return;
        };
        match typ {
            "message_start" => self.on_message_start(&v),
            "content_block_start" => self.warn_non_text_block(&v),
            "content_block_delta" => self.on_content_delta(&v),
            "message_delta" => self.on_message_delta(&v),
            "message_stop" => {
                self.pending.push_back(b"data: [DONE]\n\n".to_vec());
                self.saw_message_stop = true;
            }
            "error" => {
                let msg = v
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("claude stream error");
                self.failure = Some(UpstreamFailure::Broken(format!(
                    "claude stream error: {msg}"
                )));
            }
            "ping" => {}
            other => tracing::warn!("claude event {other} ignored"),
        }
    }

    /// thinking 等非文本内容块忽略并告警，不静默丢信号。
    fn warn_non_text_block(&self, v: &Value) {
        if let Some(block) = v.pointer("/content_block/type").and_then(Value::as_str) {
            if block != "text" {
                tracing::warn!("claude non-text content block {block} ignored");
            }
        }
    }

    fn on_message_start(&mut self, v: &Value) {
        match v
            .pointer("/message/usage/input_tokens")
            .and_then(Value::as_u64)
        {
            Some(input) => self.input_tokens = Some(input),
            None => tracing::warn!("claude message_start without input_tokens"),
        }
        self.queue(&json!({"choices": [{"delta": {"role": "assistant", "content": ""}}]}));
    }

    fn on_content_delta(&mut self, v: &Value) {
        let Some(text) = v.pointer("/delta/text").and_then(Value::as_str) else {
            tracing::warn!("claude non-text content delta ignored");
            return;
        };
        self.queue(&json!({"choices": [{"delta": {"content": text}}]}));
    }

    /// stop_reason → finish_reason；用缓存 input_tokens + output_tokens 合成 usage chunk。
    fn on_message_delta(&mut self, v: &Value) {
        if let Some(stop) = v.pointer("/delta/stop_reason").and_then(Value::as_str) {
            let finish = match stop {
                "end_turn" | "stop_sequence" => "stop",
                "max_tokens" => "length",
                _ => "stop",
            };
            self.queue(&json!({"choices": [{"delta": {}, "finish_reason": finish}]}));
        }
        let output = v.pointer("/usage/output_tokens").and_then(Value::as_u64);
        match (self.input_tokens, output) {
            (Some(input), Some(output)) => self.queue(&json!({
                "choices": [],
                "usage": {"prompt_tokens": input, "completion_tokens": output}
            })),
            _ => tracing::warn!("claude usage synthesis skipped: missing input/output tokens"),
        }
    }

    /// 上游流收束：冲刷半截事件后缺 message_stop 判定 Broken（§5.2 可观测）。
    fn on_stream_end(&mut self) {
        if let Some(residual) = self.splitter.finish() {
            self.handle_event(&residual);
        }
        self.done = true;
        if self.failure.is_none() && !self.saw_message_stop {
            self.failure = Some(UpstreamFailure::Broken(
                "claude stream ended without message_stop".into(),
            ));
        }
    }
}

impl Stream for ClaudeSseTranslator {
    type Item = Result<Vec<u8>, UpstreamFailure>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if let Some(line) = self.pending.pop_front() {
                return Poll::Ready(Some(Ok(line)));
            }
            if let Some(f) = self.failure.take() {
                self.done = true;
                return Poll::Ready(Some(Err(f)));
            }
            if self.done {
                return Poll::Ready(None);
            }
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(chunk))) => {
                    for event in self.splitter.feed(&chunk) {
                        self.handle_event(&event);
                        if self.failure.is_some() {
                            break;
                        }
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    tracing::warn!("claude stream broken: {e}");
                    self.failure = Some(e);
                }
                Poll::Ready(None) => self.on_stream_end(),
            }
        }
    }
}
