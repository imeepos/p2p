//! 已读请求帧回放适配（W3 serve 门禁前置的接线件）：外层门禁需先读请求帧
//! 检查 req_id 幂等（重启重建 settled 索引），再委托 LenderProxy::serve；
//! serve 会重读请求帧，故以 chunked CHUNK+END 两帧把已读载荷重新装入流头。
//! 不用 write_chunked 的 SINGLE 编码：wire.rs 分发支路的 finish_chunked 只认
//! CHUNK/END（FRAME_SINGLE 缺位为 W1 侧已知缝隙，见交付报告）。

use std::collections::HashMap;
use std::io::{self, Cursor};
use std::pin::Pin;
use std::task::{Context, Poll};

use llm_share_ledger::Receipt;
use p2p::BoxedStream;
use p2p_identity::Keypair;
use p2p_protocol::{write_frame, FRAME_CHUNK, FRAME_END};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// 每帧载荷上限（远小于 1MiB 帧上限，请求帧量级为 KB）。
const FRAME_DATA_CAP: usize = 64 * 1024;

/// 启动重建 settled 索引（契约 §16.6 #4 防双记账，重启幂等归属本模块）：
/// 只收本机出借侧收据（借方侧收据签名非本机，验签必败且与本机出借净差
/// 无关），逐条验签。
pub(crate) fn rebuild_settled(
    data_dir: &str,
    lender_id: &str,
    keypair: &Keypair,
) -> HashMap<String, Receipt> {
    let file = p2p_cli::llm_share::ledger::path(data_dir);
    let persisted = match p2p_cli::llm_share::ledger::load_or_empty(&file) {
        Ok(file) => file,
        Err(e) => {
            tracing::error!("llm-share ledger.json 读取失败，settled 索引按空重建: {e}");
            return HashMap::new();
        }
    };
    let mut settled = HashMap::new();
    for receipt in &persisted.receipts {
        if receipt.lender != lender_id {
            continue;
        }
        match receipt.verify(&keypair.public()) {
            Ok(()) => {
                settled.insert(receipt.req_id.clone(), receipt.clone());
            }
            Err(e) => {
                tracing::warn!(
                    req_id = %receipt.req_id,
                    "llm-share ledger.json 出借侧收据验签失败，跳过重建: {e}"
                );
            }
        }
    }
    tracing::info!(count = settled.len(), "llm-share serve settled 索引已重建");
    settled
}

/// 把载荷编码为 CHUNK(+END) 帧序列（write_frame = 变长帧头 + 类型化载荷）。
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

/// 前缀流：先吐出回放帧字节，耗尽后透传原流；写方向直通原流。
pub struct PrefixStream {
    prefix: Cursor<Vec<u8>>,
    inner: BoxedStream,
}

impl PrefixStream {
    pub fn new(prefix: Vec<u8>, inner: BoxedStream) -> Self {
        Self {
            prefix: Cursor::new(prefix),
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
