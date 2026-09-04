//! /llm-share/proxy/1 服务端流编排（§4 步骤 4-8）：读请求帧 -> 三闸 -> 上游 SSE 逐帧
//! 转发 -> 结算（解冻/双边流水/签名收据）-> 终结帧。全部终态留元数据日志；prompt 不落盘（§7.3）。

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use llm_share_ledger::{Receipt, Usage};
use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use p2p_protocol::write_chunked;

use crate::error::ErrorCode;
use crate::server::{Admitted, GateReject, IdempotencyIndex, LenderProxy};
use crate::sse::{estimate_tokens, extract_usage, SseSplitter};
use crate::upstream::{SseByteStream, UpstreamCall, UpstreamFailure};
use crate::wire::{read_request_frame, ProxyFrame, ProxyRequest};

/// 单流处理终态：code=None 即 clean done；io 为终结帧写出结果。
pub(crate) struct Outcome {
    code: Option<ErrorCode>,
    usage: Option<Usage>,
    io: std::io::Result<()>,
}

impl LenderProxy {
    /// 装配入口：swarm 侧认证后携借方 PeerId 调用（T20 真链路复用同此签名）。
    /// 返回即终结帧已写出、流随之关闭。
    pub async fn serve(&self, mut stream: BoxedStream, borrower: PeerId) -> std::io::Result<()> {
        let started = Instant::now();
        let raw = read_request_frame(&mut stream).await?;
        let outcome = self.process(&mut stream, borrower, &raw).await;
        self.log_outcome(borrower, &outcome, started);
        outcome.io
    }

    async fn process(&self, w: &mut BoxedStream, borrower: PeerId, raw: &[u8]) -> Outcome {
        let req = match ProxyRequest::parse(raw) {
            Ok(req) => req,
            Err(reason) => return self.reject(w, ErrorCode::BadRequest, reason, None).await,
        };
        match self.admit(&borrower, &req).await {
            Ok(admitted) => self.proxy_stream(w, borrower, req, admitted).await,
            Err(boxed) => {
                let GateReject {
                    code,
                    message,
                    receipt,
                } = *boxed;
                self.reject(w, code, message, receipt).await
            }
        }
    }

    /// 上游调用 + 逐帧转发；借方中途离场视同断流（上游 token 已实际消耗，仍需计费）。
    async fn proxy_stream(
        &self,
        w: &mut BoxedStream,
        borrower: PeerId,
        req: ProxyRequest,
        admitted: Admitted<'_>,
    ) -> Outcome {
        // permit 绑定到本请求：drop 时机即上游调用与转发的全部生命周期。
        let Admitted { route, permit } = admitted;
        let _permit = permit;
        let call = UpstreamCall {
            base_url: route.base_url.clone(),
            model: req.model.clone(),
            api_key: route.api_key.clone(),
            body: req.upstream_body(),
        };
        let mut sse = match route.upstream.chat(call).await {
            Ok(sse) => sse,
            Err(e) => {
                self.abort(&req.req_id, &format!("upstream connect: {e}"))
                    .await;
                let code = match &e {
                    // 连接期失败未消耗任何 token：透传状态码，不产生流水（§4 错误透传）。
                    UpstreamFailure::Rejected(_) => ErrorCode::UpstreamRejected,
                    UpstreamFailure::Broken(_) => ErrorCode::UpstreamRejected,
                };
                return self.reject(w, code, format!("upstream: {e}"), None).await;
            }
        };
        let (usage, forwarded, broken) = self.forward(w, &mut sse, &req.req_id).await;
        self.finalize(w, borrower, &req, usage, forwarded, broken)
            .await
    }

    /// 转发循环：usage 取流末 chunk（后者覆盖前者）；断流或借方离场都标记 broken。
    async fn forward(
        &self,
        w: &mut BoxedStream,
        sse: &mut SseByteStream,
        req_id: &str,
    ) -> (Option<Usage>, usize, bool) {
        let mut splitter = SseSplitter::default();
        let mut usage: Option<Usage> = None;
        let mut forwarded = 0usize;
        let mut broken = false;
        'outer: loop {
            match sse.next().await {
                Some(Ok(chunk)) => {
                    forwarded += chunk.len();
                    for event in splitter.feed(&chunk) {
                        if let Some(u) = extract_usage(&event) {
                            usage = Some(u);
                        }
                        if self.send(w, &ProxyFrame::Sse { d: event }).await.is_err() {
                            tracing::warn!(req_id, "borrower left mid-stream");
                            broken = true;
                            break 'outer;
                        }
                    }
                }
                Some(Err(e)) => {
                    tracing::warn!(req_id, "upstream stream broken: {e}");
                    broken = true;
                    break;
                }
                None => break,
            }
        }
        if let Some(event) = splitter.finish() {
            forwarded += event.len();
            if let Some(u) = extract_usage(&event) {
                usage = Some(u);
            }
            let _ = self.send(w, &ProxyFrame::Sse { d: event }).await;
        }
        (usage, forwarded, broken)
    }

    /// 结算（§4 步骤 7/8）：解冻 -> 双边流水 -> 签名收据 -> 终结帧。
    /// 断流未获 usage 时按流式估算入账，收据 estimated=true（MVP A6）。
    async fn finalize(
        &self,
        w: &mut BoxedStream,
        borrower: PeerId,
        req: &ProxyRequest,
        usage: Option<Usage>,
        forwarded: usize,
        broken: bool,
    ) -> Outcome {
        let estimated = usage.is_none();
        let usage = usage.unwrap_or_else(|| Usage {
            input: estimate_tokens(req.wire_bytes),
            output: estimate_tokens(forwarded),
        });
        if let Err(e) = self
            .holds
            .lock()
            .await
            .settle(&req.req_id, usage.input + usage.output)
        {
            // usage 超冻结额：冻结按账本语义保留，状态显式滞留待人工处置（不静默）。
            tracing::error!(req_id = %req.req_id, "settle failed, hold retained: {e}");
            return self
                .reject(w, ErrorCode::SettleFailed, format!("settle: {e}"), None)
                .await;
        }
        let receipt = self.build_receipt(borrower, req, usage, estimated);
        if let Err(e) = self
            .ledger
            .lock()
            .await
            .apply(&receipt, &self.keypair.public())
        {
            tracing::error!(req_id = %req.req_id, "ledger apply failed: {e}");
            return self
                .reject(w, ErrorCode::Internal, format!("ledger: {e}"), None)
                .await;
        }
        self.index.lock().await.settle(&req.req_id, receipt.clone());
        let frame = if broken {
            ProxyFrame::Error {
                code: ErrorCode::UpstreamStreamBroken,
                message: "upstream ended without usage; billed by estimate".into(),
                receipt: Some(receipt),
            }
        } else {
            ProxyFrame::Done { receipt }
        };
        let io = self.send(w, &frame).await;
        Outcome {
            code: broken.then_some(ErrorCode::UpstreamStreamBroken),
            usage: Some(usage),
            io,
        }
    }

    fn build_receipt(
        &self,
        borrower: PeerId,
        req: &ProxyRequest,
        usage: Usage,
        estimated: bool,
    ) -> Receipt {
        let mut receipt = Receipt {
            v: 1,
            req_id: req.req_id.clone(),
            period: self.cfg.period.clone(),
            lender: self.cfg.lender_id.clone(),
            borrower: borrower.to_string(),
            model: req.model.clone(),
            usage,
            estimated,
            upstream_hint: "openai".into(),
            ts: now_secs(),
            sig: String::new(),
        };
        // 签名失败必须显式留痕（收据是记账唯一凭据，验签方会拒绝无效签名）。
        if let Err(e) = receipt.sign(&self.keypair) {
            tracing::error!(req_id = %req.req_id, "receipt signing failed: {e}");
        }
        receipt
    }

    /// 上游连接失败等前置路径：解冻 + 清 pending + 留痕，不产生流水。
    async fn abort(&self, req_id: &str, reason: &str) {
        if let Err(e) = self.holds.lock().await.release(req_id) {
            tracing::warn!(req_id, "hold release after abort: {e}");
        }
        self.index.lock().await.pending.remove(req_id);
        tracing::warn!(req_id, reason, "proxy request aborted");
    }

    async fn reject(
        &self,
        w: &mut BoxedStream,
        code: ErrorCode,
        message: String,
        receipt: Option<Receipt>,
    ) -> Outcome {
        let io = self
            .send(
                w,
                &ProxyFrame::Error {
                    code,
                    message,
                    receipt,
                },
            )
            .await;
        Outcome {
            code: Some(code),
            usage: None,
            io,
        }
    }

    async fn send(&self, w: &mut BoxedStream, frame: &ProxyFrame) -> std::io::Result<()> {
        let bytes = serde_json::to_vec(frame)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        write_chunked(w, &bytes).await
    }

    fn log_outcome(&self, borrower: PeerId, outcome: &Outcome, started: Instant) {
        let code = outcome
            .code
            .as_ref()
            .map(|c| format!("{c:?}"))
            .unwrap_or_else(|| "done".into());
        match &outcome.usage {
            Some(u) => tracing::info!(
                lender = %self.cfg.lender_id, borrower = %borrower,
                usage_in = u.input, usage_out = u.output,
                elapsed_ms = started.elapsed().as_millis() as u64,
                outcome = %code, "proxy request finished"
            ),
            None => tracing::info!(
                lender = %self.cfg.lender_id, borrower = %borrower,
                elapsed_ms = started.elapsed().as_millis() as u64,
                outcome = %code, "proxy request finished"
            ),
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl IdempotencyIndex {
    /// 结算落位：移出在途、登记已结算（重放回传用）。
    pub(crate) fn settle(&mut self, req_id: &str, receipt: Receipt) {
        self.pending.remove(req_id);
        self.settled.insert(req_id.to_string(), receipt);
    }
}
