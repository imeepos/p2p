//! ClaudeUpstream 单元测试：进程内字节流构造 mock Claude SSE，禁外网（§5.2）。
//! 覆盖请求翻译 roundtrip、SSE 跨块切分、usage 合成、finish_reason 映射、
//! baseUrl 归一、断流/畸形事件映射、protocol_hint。

use futures::StreamExt;
use serde_json::{json, Value};

use super::response::ClaudeSseTranslator;
use super::{normalize_base_url, translate_request, ClaudeUpstream, TranslateError};
use crate::sse::extract_usage;
use crate::upstream::{Upstream, UpstreamFailure};

fn claude_stream(chunks: Vec<&'static str>) -> ClaudeSseTranslator {
    let iter = chunks
        .into_iter()
        .map(|c| c.as_bytes().to_vec())
        .map(Ok::<_, UpstreamFailure>);
    ClaudeSseTranslator::new(futures::stream::iter(iter).boxed())
}

async fn collect_lines(mut t: ClaudeSseTranslator) -> Result<Vec<String>, UpstreamFailure> {
    let mut lines = Vec::new();
    while let Some(item) = t.next().await {
        lines.push(String::from_utf8(item?).expect("utf8"));
    }
    Ok(lines)
}

fn parse_data_line(line: &str) -> Value {
    let payload = line.trim().strip_prefix("data: ").expect("data line");
    serde_json::from_str(payload).expect("json")
}

#[test]
fn translate_request_merges_system_and_filters_messages() {
    let body = json!({
        "model": "claude-3-5-sonnet",
        "max_tokens": 512,
        "stream": true,
        "temperature": 0.7,
        "stop": "END",
        "messages": [
            {"role": "system", "content": "you are helpful"},
            {"role": "system", "content": "answer in chinese"},
            {"role": "user", "content": "ping"},
            {"role": "assistant", "content": "pong"},
        ]
    });
    let out = translate_request(&body).expect("translate");
    assert_eq!(out["model"], "claude-3-5-sonnet");
    assert_eq!(out["max_tokens"], 512);
    assert_eq!(out["stream"], true);
    assert_eq!(out["temperature"], 0.7);
    assert_eq!(out["stop_sequences"], json!(["END"]));
    assert_eq!(out["system"], "you are helpful\nanswer in chinese");
    let roles: Vec<_> = out["messages"]
        .as_array()
        .expect("messages array")
        .iter()
        .map(|m| m["role"].as_str().expect("role"))
        .collect();
    assert_eq!(roles, ["user", "assistant"]);
    assert!(out.get("stop").is_none(), "stop 已映射为 stop_sequences");
}

#[test]
fn unsupported_params_omitted_and_top_p_wins() {
    let body = json!({
        "model": "m", "max_tokens": 10,
        "messages": [{"role": "user", "content": "hi"}],
        "temperature": 0.5, "top_p": 0.9,
        "frequency_penalty": 0.1, "presence_penalty": 0.2,
        "logprobs": true, "n": 2, "seed": 1, "response_format": {"type": "json"}
    });
    let out = translate_request(&body).expect("translate");
    assert_eq!(out["top_p"], 0.9, "top_p 优先");
    assert!(out.get("temperature").is_none(), "并存时 temperature 省略");
    for key in [
        "frequency_penalty",
        "presence_penalty",
        "logprobs",
        "n",
        "seed",
        "response_format",
    ] {
        assert!(out.get(key).is_none(), "{key} 必须显式省略");
    }
}

#[test]
fn tools_and_tool_role_rejected_structurally() {
    let with_tools = json!({
        "model": "m", "max_tokens": 10,
        "tools": [{"type": "function", "function": {"name": "f"}}],
        "messages": [{"role": "user", "content": "hi"}]
    });
    assert!(matches!(
        translate_request(&with_tools),
        Err(TranslateError::ToolUnsupported(_))
    ));
    let with_tool_role = json!({
        "model": "m", "max_tokens": 10,
        "messages": [{"role": "tool", "tool_call_id": "t1", "content": "ok"}]
    });
    assert!(matches!(
        translate_request(&with_tool_role),
        Err(TranslateError::ToolUnsupported(_))
    ));
    let with_tool_calls = json!({
        "model": "m", "max_tokens": 10,
        "messages": [{"role": "assistant", "content": "", "tool_calls": [{"id": "1"}]}]
    });
    assert!(matches!(
        translate_request(&with_tool_calls),
        Err(TranslateError::ToolUnsupported(_))
    ));
}

#[test]
fn missing_required_fields_rejected() {
    let no_messages = json!({"model": "m", "max_tokens": 10});
    assert!(matches!(
        translate_request(&no_messages),
        Err(TranslateError::MissingField("messages"))
    ));
    let no_tokens = json!({"model": "m", "messages": [{"role": "user", "content": "hi"}]});
    assert!(matches!(
        translate_request(&no_tokens),
        Err(TranslateError::MissingField("max_tokens"))
    ));
}

#[test]
fn non_text_content_blocks_filtered_to_text() {
    let body = json!({
        "model": "m", "max_tokens": 10,
        "messages": [
            {"role": "user", "content": [
                {"type": "text", "text": "describe"},
                {"type": "image_url", "image_url": {"url": "https://x/y.png"}}
            ]}
        ]
    });
    let out = translate_request(&body).expect("translate");
    assert_eq!(
        out["messages"][0]["content"],
        json!([{"type": "text", "text": "describe"}])
    );
}

#[test]
fn base_url_normalization() {
    assert_eq!(
        normalize_base_url("https://api.anthropic.com"),
        "https://api.anthropic.com/v1/messages"
    );
    assert_eq!(
        normalize_base_url("https://api.anthropic.com/v1"),
        "https://api.anthropic.com/v1/messages"
    );
    assert_eq!(
        normalize_base_url("https://api.anthropic.com/v1/"),
        "https://api.anthropic.com/v1/messages"
    );
    assert_eq!(
        normalize_base_url("https://gate.example.com/"),
        "https://gate.example.com/v1/messages"
    );
}

#[tokio::test]
async fn sse_translated_across_chunk_boundaries() {
    // 一事件跨三块 + 一块含两事件：message_start 被切碎，delta/stop 同块。
    let t = claude_stream(vec![
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":25",
        ",\"output_tokens\":1}}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",",
        "\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":4}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    ]);
    let lines = collect_lines(t).await.expect("stream");
    assert_eq!(
        parse_data_line(&lines[0]),
        json!({"choices": [{"delta": {"role": "assistant", "content": ""}}]})
    );
    assert_eq!(
        parse_data_line(&lines[1]),
        json!({"choices": [{"delta": {"content": "Hello"}}]})
    );
    assert_eq!(
        parse_data_line(&lines[2]),
        json!({"choices": [{"delta": {}, "finish_reason": "stop"}]})
    );
    assert_eq!(
        parse_data_line(&lines[3]),
        json!({"choices": [], "usage": {"prompt_tokens": 25, "completion_tokens": 4}})
    );
    assert_eq!(lines[4].trim(), "data: [DONE]");
}

#[tokio::test]
async fn synthesized_usage_hits_extract_usage() {
    let t = claude_stream(vec![
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":9}}}\n\n",
        "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"max_tokens\"},\"usage\":{\"output_tokens\":7}}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    ]);
    let lines = collect_lines(t).await.expect("stream");
    let usage_line = lines
        .iter()
        .find(|l| l.contains("\"usage\""))
        .expect("usage line");
    let u = extract_usage(usage_line).expect("extract");
    assert_eq!((u.input, u.output), (9, 7));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("\"finish_reason\":\"length\"")),
        "max_tokens → length"
    );
}

#[tokio::test]
async fn stream_ending_without_message_stop_is_broken() {
    let t = claude_stream(vec![
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":3}}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"partial\"}}\n\n",
    ]);
    let mut t = t;
    let mut broken = false;
    while let Some(item) = t.next().await {
        if let Err(UpstreamFailure::Broken(msg)) = item {
            assert!(msg.contains("message_stop"), "{msg}");
            broken = true;
        }
    }
    assert!(broken, "缺 message_stop 必须显式 Broken");
}

#[tokio::test]
async fn malformed_event_is_broken() {
    let t = claude_stream(vec![
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":3}}}\n\n",
        "event: garbage\ndata: {not-json}\n\n",
    ]);
    let mut t = t;
    let mut broken = false;
    while let Some(item) = t.next().await {
        if let Err(UpstreamFailure::Broken(msg)) = item {
            assert!(msg.contains("not json"), "{msg}");
            broken = true;
        }
    }
    assert!(broken, "畸形 JSON 必须显式 Broken");
}

#[tokio::test]
async fn thinking_block_ignored_without_content_line() {
    let t = claude_stream(vec![
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":2}}}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"content_block\":{\"type\":\"thinking\",\"thinking\":\"hmm\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"hmm\"}}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    ]);
    let lines = collect_lines(t).await.expect("stream");
    assert_eq!(
        lines.len(),
        2,
        "仅 role delta + [DONE]，thinking 不产出内容行"
    );
}

#[test]
fn protocol_hints() {
    let claude = ClaudeUpstream::new().expect("client");
    assert_eq!(claude.protocol_hint(), "claude");
    let http = crate::upstream_http::HttpUpstream::new().expect("client");
    assert_eq!(http.protocol_hint(), "openai", "HttpUpstream 保持缺省");
}
