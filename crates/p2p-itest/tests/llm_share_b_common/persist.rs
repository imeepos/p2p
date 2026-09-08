//! 重启幂等夹具（契约 §16.6 #4，镜像 W3 src-tauri serve/replay 接缝）：
//! 外层读请求帧查重建后的 settled 索引，命中即结构化 DuplicateReqId 拒绝；
//! 未命中以 CHUNK(+END) 回放帧委托 LenderProxy::serve，结算后 drain 收据
//! append 到 llm-share/ledger.json（防双记账红线）。
#![allow(dead_code)] // 共享夹具：各测试二进制按需取用

use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use llm_share_ledger::Receipt;
use llm_share_proxy::server::LenderProxy;
use llm_share_proxy::wire::read_request_frame;
use llm_share_proxy::{ErrorCode, ProxyFrame, ProxyRequest, PROTOCOL_ID};
use p2p::{BoxedStream, PeerId, ProtocolHandler, ProtocolId};
use p2p_cli::llm_share::ledger;
use p2p_identity::Keypair;
use p2p_protocol::{write_chunked, write_frame, FRAME_CHUNK, FRAME_END};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// 每帧载荷上限（请求帧 KB 量级，远小于 1MiB 帧上限）。
const FRAME_DATA_CAP: usize = 64 * 1024;

/// 已读载荷编码为 CHUNK(+END) 帧序列（与产品 replay 同形，write_frame = 变长帧头）。
pub async fn encode_replay_frames(payload: &[u8]) -> io::Result<Vec<u8>> {
    let mut prefix = Vec::with_capacity(payload.len() + 16);
    let mut rest = payload;
    if rest.is_empty() {
        write_frame(&mut prefix, &[FRAME_END]).await?;
        return Ok(prefix);
    }
    while rest.len() > FRAME_DATA_CAP {
        let (head, tail) = rest.split_at(FRAME_DATA_CAP);
        let mut frame = Vec::with_capacity(FRAME_DATA_CAP + 1);
        frame.push(FRAME_CHUNK);
        frame.extend_from_slice(head);
        write_frame(&mut prefix, &frame).await?;
        rest = tail;
    }
    let mut frame = Vec::with_capacity(rest.len() + 1);
    frame.push(FRAME_CHUNK);
    frame.extend_from_slice(rest);
    write_frame(&mut prefix, &frame).await?;
    write_frame(&mut prefix, &[FRAME_END]).await?;
    Ok(prefix)
}

/// 前缀流：先吐回放帧字节，耗尽后透传原流；写方向直通原流。
pub struct PrefixStream {
    prefix: io::Cursor<Vec<u8>>,
    inner: BoxedStream,
}

impl PrefixStream {
    pub fn new(prefix: Vec<u8>, inner: BoxedStream) -> Self {
        Self {
            prefix: io::Cursor::new(prefix),
            inner,
        }
    }
}

impl AsyncRead for PrefixStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let pos = self.prefix.position() as usize;
        let remaining = self.prefix.get_ref().len().saturating_sub(pos);
        if remaining > 0 {
            let take = remaining.min(buf.remaining());
            buf.put_slice(&self.prefix.get_ref()[pos..pos + take]);
            self.prefix.set_position((pos + take) as u64);
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for PrefixStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// 持久化 serve 门禁：settled 索引启动重建 + 结算收据落 ledger.json。
pub struct PersistProxyHandler {
    proxy: Arc<LenderProxy>,
    data_dir: String,
    settled: HashMap<String, Receipt>,
}

impl PersistProxyHandler {
    pub fn new(proxy: Arc<LenderProxy>, data_dir: &str, keypair: &Keypair) -> Self {
        let settled = Self::rebuild(data_dir, proxy.lender_id(), &keypair.public());
        Self {
            proxy,
            data_dir: data_dir.to_owned(),
            settled,
        }
    }

    /// 只收本机出借侧收据（借方侧签名验签必败），逐条验签后入索引。
    fn rebuild(data_dir: &str, lender_id: &str, pubkey: &[u8; 32]) -> HashMap<String, Receipt> {
        let persisted = match ledger::load_or_empty(&ledger::path(data_dir)) {
            Ok(file) => file,
            Err(e) => {
                eprintln!("llm-share ledger.json 读取失败，settled 按空重建: {e}");
                return HashMap::new();
            }
        };
        persisted
            .receipts
            .into_iter()
            .filter(|receipt| receipt.lender == lender_id)
            .filter_map(|receipt| match receipt.verify(pubkey) {
                Ok(()) => Some((receipt.req_id.clone(), receipt)),
                Err(e) => {
                    eprintln!("llm-share ledger.json 收据验签失败跳过: {e}");
                    None
                }
            })
            .collect()
    }

    async fn persist_receipts(&self) {
        for receipt in self.proxy.receipts().await {
            if let Err(e) = ledger::record(&self.data_dir, &receipt) {
                eprintln!("llm-share ledger.json 收据落盘失败 {}: {e}", receipt.req_id);
            }
        }
    }
}

#[async_trait]
impl ProtocolHandler for PersistProxyHandler {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(PROTOCOL_ID).expect("protocol id")
    }

    async fn handle_inbound(&self, peer: PeerId, mut stream: BoxedStream) -> io::Result<()> {
        let raw = read_request_frame(&mut stream).await?;
        if let Ok(req) = ProxyRequest::parse(&raw) {
            if let Some(stale) = self.settled.get(&req.req_id) {
                eprintln!("b7: 重放命中重建索引 req_id={}", req.req_id);
                let frame = ProxyFrame::Error {
                    code: ErrorCode::DuplicateReqId,
                    message: format!("req_id {} already settled", req.req_id),
                    receipt: Some(stale.clone()),
                };
                let bytes = serde_json::to_vec(&frame)?;
                return write_chunked(&mut stream, &bytes).await;
            }
        }
        let prefix = encode_replay_frames(&raw).await?;
        let boxed: BoxedStream = Box::new(PrefixStream::new(prefix, stream));
        let io = self.proxy.serve(boxed, peer).await;
        self.persist_receipts().await;
        io
    }

    /// 裸流无认证身份上下文：fail-closed。
    async fn handle(&self, _stream: BoxedStream) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "bare stream has no authenticated borrower",
        ))
    }
}
