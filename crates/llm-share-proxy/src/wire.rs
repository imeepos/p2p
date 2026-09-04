//! /llm-share/proxy/1 线格式（§5）：请求帧 = OpenAI chat completions JSON + req_id；
//! 应答为帧序列：若干 Sse 数据帧 + 恰一帧终结（Done/Error）。全部经底座 chunked 通道承载。

use llm_share_ledger::Receipt;
use p2p_mux::BoxedStream;
use p2p_protocol::{
    read_chunked, read_frame, FRAME_CHUNK, FRAME_END, FRAME_SINGLE, MAX_MESSAGE_SIZE,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::ErrorCode;

pub const PROTOCOL_ID: &str = "/llm-share/proxy/1";

/// 服务端读请求帧，兼容两种接线（wire-protocol：每条流首帧为协议 ID）：
/// - 裸流装配：首帧为协议 ID（须匹配本协议），随后为 chunked 请求帧；
/// - dispatch_inbound 已消费协议 ID：首帧即 chunked 请求帧本体。
///
/// 以首字节判别（'/' 为协议 ID，0x00-0x02 为 chunked 类型头），无猜测降级。
pub async fn read_request_frame(stream: &mut BoxedStream) -> std::io::Result<Vec<u8>> {
    let first = read_frame(stream).await?;
    match first.first().copied() {
        Some(b'/') => {
            let id = String::from_utf8(first).map_err(|_| wire_err("protocol id not utf-8"))?;
            if id != PROTOCOL_ID {
                return Err(wire_err(format!("unexpected protocol {id}")));
            }
            read_chunked(stream).await
        }
        Some(head) => {
            if !matches!(head, FRAME_SINGLE | FRAME_CHUNK | FRAME_END) {
                return Err(wire_err("request frame missing chunked type header"));
            }
            finish_chunked(stream, first).await
        }
        None => Err(wire_err("request frame missing chunked type header")),
    }
}

/// 以已读首帧为起点的 chunked 重组（语义对齐 p2p-protocol::read_chunked）。
async fn finish_chunked(stream: &mut BoxedStream, first: Vec<u8>) -> std::io::Result<Vec<u8>> {
    let mut msg = Vec::new();
    let mut frame = first;
    loop {
        let Some(head) = frame.first().copied() else {
            return Err(wire_err("chunked frame missing type byte"));
        };
        msg.extend_from_slice(&frame[1..]);
        let total = msg.len() as u64;
        if total > MAX_MESSAGE_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                p2p_protocol::ProtocolError::MessageTooLarge(total),
            ));
        }
        match head {
            FRAME_END => return Ok(msg),
            FRAME_CHUNK => frame = read_frame(stream).await?,
            _ => {
                return Err(wire_err(format!(
                    "unexpected chunked frame type {head:#04x}"
                )))
            }
        }
    }
}

fn wire_err(msg: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, msg.into())
}

/// 代理请求：body 原样承载 OpenAI 字段；req_id/model/max_tokens 为闸门与冻结的必需字段。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProxyRequest {
    pub req_id: String,
    pub body: Value,
    pub model: String,
    pub max_tokens: u64,
    /// 请求帧线字节长度：输入 token 估算的确定性基准（§4 est = f(输入估算, max_tokens)）。
    #[serde(default)]
    pub wire_bytes: usize,
}

impl ProxyRequest {
    /// 解析并校验必需字段；任何缺失都以可述原因拒绝（BadRequest）。
    pub fn parse(raw: &[u8]) -> Result<Self, String> {
        let v: Value = serde_json::from_slice(raw).map_err(|e| format!("request not json: {e}"))?;
        let req_id = v.get("req_id").and_then(Value::as_str).unwrap_or_default();
        if req_id.is_empty() {
            return Err("missing req_id".into());
        }
        let model = v.get("model").and_then(Value::as_str).unwrap_or_default();
        if model.is_empty() {
            return Err("missing model".into());
        }
        let max_tokens = v
            .get("max_tokens")
            .and_then(Value::as_u64)
            .or_else(|| v.get("max_completion_tokens").and_then(Value::as_u64))
            .unwrap_or(0);
        if max_tokens == 0 {
            return Err("missing max_tokens".into());
        }
        Ok(Self {
            req_id: req_id.into(),
            model: model.into(),
            max_tokens,
            body: v,
            wire_bytes: raw.len(),
        })
    }

    /// 上游调用体：剥离代理字段 req_id（OpenAI 上游不识别）。
    pub fn upstream_body(&self) -> Value {
        let mut body = self.body.clone();
        if let Some(obj) = body.as_object_mut() {
            obj.remove("req_id");
        }
        body
    }
}

/// 应答帧：Sse 携带上游事件原文；Done/Error 恰为终结帧。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ProxyFrame {
    Sse {
        d: String,
    },
    /// 结算完成，收据为记账唯一凭据（§5.1）。
    Done {
        receipt: Receipt,
    },
    /// 终结失败；断流计费（UpstreamStreamBroken）时携带估算收据。
    Error {
        code: ErrorCode,
        message: String,
        #[serde(default)]
        receipt: Option<Receipt>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        serde_json::json!({
            "req_id": "r-1", "model": "gpt-4o", "max_tokens": 128,
            "messages": [{ "role": "user", "content": "ping" }], "stream": true
        })
    }

    #[test]
    fn parse_extracts_gates_fields() {
        let raw = serde_json::to_vec(&sample()).expect("json");
        let req = ProxyRequest::parse(&raw).expect("valid");
        assert_eq!(req.req_id, "r-1");
        assert_eq!(req.model, "gpt-4o");
        assert_eq!(req.max_tokens, 128);
        assert_eq!(req.wire_bytes, raw.len());
    }

    #[test]
    fn upstream_body_strips_proxy_field() {
        let raw = serde_json::to_vec(&sample()).expect("json");
        let req = ProxyRequest::parse(&raw).expect("valid");
        assert!(req.body.get("req_id").is_some());
        assert!(req.upstream_body().get("req_id").is_none());
        assert_eq!(req.upstream_body()["model"], "gpt-4o");
    }

    #[test]
    fn missing_required_fields_rejected() {
        let mut no_tokens = sample();
        no_tokens.as_object_mut().expect("obj").remove("max_tokens");
        assert!(ProxyRequest::parse(&serde_json::to_vec(&no_tokens).expect("json")).is_err());
        let mut no_id = sample();
        no_id.as_object_mut().expect("obj").remove("req_id");
        assert!(ProxyRequest::parse(&serde_json::to_vec(&no_id).expect("json")).is_err());
        assert!(ProxyRequest::parse(b"not-json").is_err());
    }
}
