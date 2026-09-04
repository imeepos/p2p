//! 拨号客户端（借方侧，§4 步骤 1/6/9）：经 StreamFactory 开流发请求帧，
//! 收 SSE 转发帧与终结帧；终结收据强制验签，验签失败显式报错不吞。

use std::time::Duration;

use llm_share_ledger::Receipt;
use p2p_identity::PeerId;
use p2p_protocol::{open_with_protocol, read_chunked, write_chunked, ProtocolId, StreamFactory};
use tokio::time;

use crate::error::{ErrorCode, ProxyClientError};
use crate::wire::{ProxyFrame, ProxyRequest, PROTOCOL_ID};

/// 借方视角的事件序列：Sse 转发帧若干 + 恰一终结事件。
#[derive(Debug)]
pub enum ProxyEvent {
    /// 上游 SSE 事件原文。
    Sse(String),
    /// 结算完成；stream_broken=true 表示上游断流，收据为估算（estimated=true）。
    Finished {
        receipt: Receipt,
        stream_broken: bool,
    },
    /// 结构化拒绝；重放去重时 receipt 携带原收据（同次结果，MVP A4）。
    Rejected {
        code: ErrorCode,
        message: String,
        receipt: Option<Receipt>,
    },
}

#[derive(Clone)]
pub struct ProxyClient<F: StreamFactory> {
    factory: F,
}

impl<F: StreamFactory> ProxyClient<F> {
    pub fn new(factory: F) -> Self {
        Self { factory }
    }

    /// 拨号 -> 请求 -> 收齐终结事件；一个 timeout 覆盖全程，收据验签失败即错。
    pub async fn call(
        &self,
        peer: PeerId,
        req: &ProxyRequest,
        lender_pubkey: [u8; 32],
        timeout: Duration,
    ) -> Result<Vec<ProxyEvent>, ProxyClientError> {
        match time::timeout(timeout, self.exchange(peer, req, lender_pubkey)).await {
            Ok(res) => res,
            Err(_) => Err(ProxyClientError::Timeout(timeout)),
        }
    }

    async fn exchange(
        &self,
        peer: PeerId,
        req: &ProxyRequest,
        lender_pubkey: [u8; 32],
    ) -> Result<Vec<ProxyEvent>, ProxyClientError> {
        let id = ProtocolId::new(PROTOCOL_ID).map_err(|e| ProxyClientError::Wire(e.to_string()))?;
        let opened = self.factory.open_stream(&peer, &id).await?;
        let mut stream = open_with_protocol(opened, &id).await?;
        let payload = serde_json::to_vec(req).map_err(|e| ProxyClientError::Wire(e.to_string()))?;
        write_chunked(&mut stream, &payload).await?;
        let mut events = Vec::new();
        loop {
            let raw = read_chunked(&mut stream).await?;
            let frame: ProxyFrame = serde_json::from_slice(&raw)
                .map_err(|e| ProxyClientError::Wire(format!("frame: {e}")))?;
            match frame {
                ProxyFrame::Sse { d } => events.push(ProxyEvent::Sse(d)),
                ProxyFrame::Done { receipt } => {
                    verify(&receipt, &lender_pubkey)?;
                    events.push(ProxyEvent::Finished {
                        receipt,
                        stream_broken: false,
                    });
                    return Ok(events);
                }
                ProxyFrame::Error {
                    code,
                    message,
                    receipt,
                } => {
                    if code == ErrorCode::UpstreamStreamBroken {
                        let receipt = receipt.ok_or_else(|| {
                            ProxyClientError::Wire("broken stream without receipt".into())
                        })?;
                        verify(&receipt, &lender_pubkey)?;
                        events.push(ProxyEvent::Finished {
                            receipt,
                            stream_broken: true,
                        });
                        return Ok(events);
                    }
                    events.push(ProxyEvent::Rejected {
                        code,
                        message,
                        receipt,
                    });
                    return Ok(events);
                }
            }
        }
    }
}

fn verify(receipt: &Receipt, lender_pubkey: &[u8; 32]) -> Result<(), ProxyClientError> {
    receipt
        .verify(lender_pubkey)
        .map_err(|_| ProxyClientError::BadReceipt(receipt.req_id.clone()))
}
