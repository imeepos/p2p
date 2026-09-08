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

/// 以已读首帧为起点的 chunked 重组（语义对齐 p2p-protocol::read_chunked）：
/// FRAME_SINGLE 仅在无累积载荷时合法（整条消息一帧装下即到消息末尾）。
async fn finish_chunked(stream: &mut BoxedStream, first: Vec<u8>) -> std::io::Result<Vec<u8>> {
    let mut msg: Vec<u8> = Vec::new();
    let mut frame = first;
    loop {
        let Some(head) = frame.first().copied() else {
            return Err(wire_err("chunked frame missing type byte"));
        };
        let data = &frame[1..];
        let total = msg.len() as u64 + data.len() as u64;
        if total > MAX_MESSAGE_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                p2p_protocol::ProtocolError::MessageTooLarge(total),
            ));
        }
        match head {
            FRAME_SINGLE if msg.is_empty() => return Ok(data.to_vec()),
            FRAME_END => {
                msg.extend_from_slice(data);
                return Ok(msg);
            }
            FRAME_CHUNK => {
                msg.extend_from_slice(data);
                frame = read_frame(stream).await?;
            }
            _ => {
                return Err(wire_err(format!(
                    "unexpected chunked frame type {head:#04x} after {} bytes",
                    msg.len()
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
        let req_id = v
            .get("req_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if req_id.is_empty() {
            return Err("missing req_id".into());
        }
        let model = v
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
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
        // 双形态兼容：ProxyClient 序列化信封（含 body 字段，内层才是 OpenAI 体），
        // 测试/手工路径为扁平体（OpenAI 字段 + 顶层 req_id）。真实上游只认后者。
        let body = match v.get("body") {
            Some(inner) => inner.clone(),
            None => v,
        };
        Ok(Self {
            req_id,
            model,
            max_tokens,
            body,
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
