//! QUIC 场景复用：直通 quinn 原生双向流（design §8），仅叠加本端开流上限。

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use quinn::{Connection, RecvStream, SendStream};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::Semaphore;

use super::limited::Limited;
use super::{BoxedStream, MuxControl};

/// quinn 连接上的复用控制器。
pub struct QuicMux {
    conn: Connection,
    open_permits: Arc<Semaphore>,
}

impl QuicMux {
    pub fn new(conn: Connection, max_open_streams: usize) -> Self {
        Self { conn, open_permits: Arc::new(Semaphore::new(max_open_streams)) }
    }
}

#[async_trait::async_trait]
impl MuxControl for QuicMux {
    async fn open_stream(&self) -> io::Result<BoxedStream> {
        let permit = self.open_permits.clone().acquire_owned().await.map_err(|_| {
            io::Error::new(io::ErrorKind::BrokenPipe, "mux closed")
        })?;
        let (send, recv) = self.conn.open_bi().await.map_err(transport_err)?;
        Ok(Box::new(Limited::new(QuicStream { send, recv }, permit)))
    }

    async fn accept_stream(&self) -> Option<BoxedStream> {
        let (send, recv) = self.conn.accept_bi().await.ok()?;
        Some(Box::new(QuicStream { send, recv }))
    }
}

/// quinn SendStream + RecvStream 的组合对象，适配 tokio AsyncRead/AsyncWrite。
struct QuicStream {
    send: SendStream,
    recv: RecvStream,
}

impl AsyncRead for QuicStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        // 经栈上中转缓冲调用 quinn 的 poll_read，保证 ReadBuf 初始化语义安全
        let mut tmp = [0u8; 4096];
        match this.recv.poll_read(cx, &mut tmp) {
            Poll::Ready(Ok(0)) => Poll::Ready(Ok(())), // 对端 FIN
            Poll::Ready(Ok(n)) => {
                buf.put_slice(&tmp[..n]);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(transport_err(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for QuicStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().send).poll_write(cx, buf).map_err(transport_err)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // finish 发送 FIN；流被对端 stop 或连接已关时无法再关，debug 留痕
        if let Err(e) = self.get_mut().send.finish() {
            tracing::debug!(error = %e, "quinn finish on closed stream");
        }
        Poll::Ready(Ok(()))
    }
}

fn transport_err(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::ConnectionReset, e.to_string())
}
