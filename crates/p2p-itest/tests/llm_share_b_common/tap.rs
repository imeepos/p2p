//! P2P 线字节观测夹具（B5 泄露面）：包装任意 handler，流经 RecordingStream
//! 把读写双向字节 tee 进共享缓冲；断言只关心内容不关心方向。

use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use async_trait::async_trait;
use p2p::{BoxedStream, PeerId, ProtocolHandler, ProtocolId};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// 双向线字节缓冲。
pub type WireLog = Arc<Mutex<Vec<u8>>>;

/// 包装任意 handler：入站流经 RecordingStream 后透传。
pub struct TapHandler {
    inner: Arc<dyn ProtocolHandler>,
    log: WireLog,
}

impl TapHandler {
    pub fn new(inner: Arc<dyn ProtocolHandler>, log: WireLog) -> Self {
        Self { inner, log }
    }
}

#[async_trait]
impl ProtocolHandler for TapHandler {
    fn protocol(&self) -> ProtocolId {
        self.inner.protocol()
    }

    async fn handle_inbound(&self, peer: PeerId, stream: BoxedStream) -> io::Result<()> {
        let tapped: BoxedStream = Box::new(RecordingStream {
            inner: stream,
            log: self.log.clone(),
        });
        self.inner.handle_inbound(peer, tapped).await
    }

    async fn handle(&self, stream: BoxedStream) -> io::Result<()> {
        let tapped: BoxedStream = Box::new(RecordingStream {
            inner: stream,
            log: self.log.clone(),
        });
        self.inner.handle(tapped).await
    }
}

/// 双向 tee 流：读到的增量与写出的字节都进 log，锁毒化显式报错不静默。
struct RecordingStream {
    inner: BoxedStream,
    log: WireLog,
}

impl RecordingStream {
    fn record(&self, bytes: &[u8]) -> io::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        let mut log = self
            .log
            .lock()
            .map_err(|_| io::Error::other("wire log poisoned"))?;
        log.extend_from_slice(bytes);
        Ok(())
    }
}

impl AsyncRead for RecordingStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let filled_before = buf.filled().len();
        match Pin::new(&mut self.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                let new_bytes = &buf.filled()[filled_before..];
                if let Err(e) = self.record(new_bytes) {
                    return Poll::Ready(Err(e));
                }
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

impl AsyncWrite for RecordingStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if let Err(e) = self.record(buf) {
            return Poll::Ready(Err(e));
        }
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
