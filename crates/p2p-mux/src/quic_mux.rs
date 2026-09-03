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
        Self {
            conn,
            open_permits: Arc::new(Semaphore::new(max_open_streams)),
        }
    }
}

#[async_trait::async_trait]
impl MuxControl for QuicMux {
    async fn open_stream(&self) -> io::Result<BoxedStream> {
        // 吞错豁免：信号量 close() 本 crate 从不调用，AcquireError 仅在
        // close 后出现（不可达分支）；文本 BrokenPipe 即完整语义
        let permit = self
            .open_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "mux closed"))?;
        let (send, recv) = self.conn.open_bi().await.map_err(transport_err)?;
        Ok(Box::new(Limited::new(QuicStream { send, recv }, permit)))
    }

    async fn accept_stream(&self) -> Option<BoxedStream> {
        let (send, recv) = match self.conn.accept_bi().await {
            Ok(stream) => stream,
            // 原 .ok()? 静默吞错，E7-K2 补观测信号：连接收敛（对端挂断/
            // 空闲超时/本地 close）本就落 None，此处 debug 留痕不刷屏
            Err(e) => {
                tracing::debug!(error = %e, "quic accept_bi terminated");
                return None;
            }
        };
        Some(Box::new(QuicStream { send, recv }))
    }

    fn close(&self) {
        // 0 号应用关闭码：本端主动挂断专用，对端 accept_bi 随之报错收敛
        self.conn.close(quinn::VarInt::from_u32(0), b"hangup");
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
        // 经栈上中转缓冲调用 quinn 的 poll_read，读取量以调用方缓冲为上限
        let cap = buf.remaining().min(4096);
        if cap == 0 {
            return Poll::Ready(Ok(()));
        }
        let mut tmp = [0u8; 4096];
        match this.recv.poll_read(cx, &mut tmp[..cap]) {
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
        Pin::new(&mut self.get_mut().send)
            .poll_write(cx, buf)
            .map_err(transport_err)
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

/// E5 登记项修复（E7-K2 错误链保真）：quinn 错误整体装箱为 io::Error 载荷，
/// 内层 ConnectionError/ReadError/WriteError 经 `io::Error::source` downcast
/// 可原样还原类型与文案，拒绝 to_string 拍平。
/// io kind 维持 ConnectionReset：本修复只补错误链，不改变既有错误语义。
fn transport_err<E>(e: E) -> io::Error
where
    E: std::error::Error + Send + Sync + 'static,
{
    io::Error::new(
        io::ErrorKind::ConnectionReset,
        super::ChainedPayload { inner: e },
    )
}
