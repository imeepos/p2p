//! TCP 场景复用：yamux 封装（design §5.1），独立驱动任务持有连接。
//!
//! yamux 基于 futures-io trait，经 tokio-util compat 与底座 tokio 字节流互转。

use std::future::poll_fn;
use std::io;
use std::sync::Arc;
use std::task::Poll;

use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio_util::compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};

use super::limited::Limited;
use super::{BoxedStream, MuxControl};

type OpenReply = oneshot::Sender<io::Result<BoxedStream>>;

/// yamux 连接上的复用控制器。
pub struct YamuxMux {
    open_tx: mpsc::Sender<OpenReply>,
    inbound_rx: tokio::sync::Mutex<mpsc::Receiver<BoxedStream>>,
    open_permits: Arc<Semaphore>,
    /// 全部句柄丢弃时发送端归零，驱动任务随即关闭连接（仅持有，不主动发送）。
    _close_tx: mpsc::Sender<()>,
}

impl YamuxMux {
    /// is_initiator：主动拨号侧传 true（决定 yamux stream id 奇偶，两端必须互异）。
    pub fn new(io: BoxedStream, is_initiator: bool, max_open_streams: usize) -> Self {
        let mode = if is_initiator { yamux::Mode::Client } else { yamux::Mode::Server };
        let mut cfg = yamux::Config::default();
        // 连接级流数硬上限写入协议配置（对端开流同样受限）；须满足 <= 4096
        cfg.set_max_num_streams(max_open_streams);
        let conn = yamux::Connection::new(io.compat(), cfg, mode);

        let (open_tx, open_rx) = mpsc::channel::<OpenReply>(8);
        let (in_tx, in_rx) = mpsc::channel::<BoxedStream>(16);
        let (close_tx, close_rx) = mpsc::channel::<()>(1);
        tokio::spawn(drive(conn, open_rx, in_tx, close_rx));
        Self {
            open_tx,
            inbound_rx: tokio::sync::Mutex::new(in_rx),
            open_permits: Arc::new(Semaphore::new(max_open_streams)),
            _close_tx: close_tx,
        }
    }
}

#[async_trait::async_trait]
impl MuxControl for YamuxMux {
    async fn open_stream(&self) -> io::Result<BoxedStream> {
        let permit = self.open_permits.clone().acquire_owned().await.map_err(|_| {
            io::Error::new(io::ErrorKind::BrokenPipe, "mux closed")
        })?;
        let (tx, rx) = oneshot::channel();
        self.open_tx.clone().send(tx).await.map_err(|_| mux_closed())?;
        let stream = rx.await.map_err(|_| mux_closed())??;
        Ok(Box::new(Limited::new(stream, permit)))
    }

    async fn accept_stream(&self) -> Option<BoxedStream> {
        let mut rx = self.inbound_rx.lock().await;
        rx.recv().await.map(|s| s as BoxedStream)
    }
}

async fn drive(
    mut conn: yamux::Connection<tokio_util::compat::Compat<BoxedStream>>,
    mut open_rx: mpsc::Receiver<OpenReply>,
    inbound_tx: mpsc::Sender<BoxedStream>,
    mut close_rx: mpsc::Receiver<()>,
) {
    let mut pending_open: Option<OpenReply> = None;
    let mut open_rx_closed = false;
    loop {
        // 单点驱动：连接事件与开流请求在 select 中竞争，
        // 保证等待开流请求期间写缓冲/到达帧仍被持续轮询。
        tokio::select! {
            biased;
            _ = close_rx.recv() => return, // 句柄归零或显式关闭
            req = open_rx.recv(), if !open_rx_closed && pending_open.is_none() => {
                match req {
                    Some(reply) => pending_open = Some(reply),
                    None => open_rx_closed = true, // 句柄全部丢弃，只收流直至连接关闭
                }
            }
            flow = poll_fn(|cx| poll_once(&mut conn, &mut pending_open, cx)) => {
                match flow {
                    Flow::Inbound(stream) => {
                        if inbound_tx.send(stream).await.is_err() {
                            // 上层已不再收流：丢弃该流，drop 即复位，不留悬挂数据
                            tracing::debug!("inbound stream dropped: no accept_stream consumer");
                        }
                    }
                    Flow::Closed => return,
                    Flow::Error(e) => {
                        tracing::warn!(error = %e, "yamux connection terminated");
                        return;
                    }
                }
            }
        }
    }
}

enum Flow {
    Inbound(BoxedStream),
    Closed,
    Error(yamux::ConnectionError),
}

fn poll_once(
    conn: &mut yamux::Connection<tokio_util::compat::Compat<BoxedStream>>,
    pending_open: &mut Option<OpenReply>,
    cx: &mut std::task::Context<'_>,
) -> Poll<Flow> {
    match conn.poll_next_inbound(cx) {
        Poll::Ready(Some(Ok(stream))) => {
            return Poll::Ready(Flow::Inbound(Box::new(stream.compat())))
        }
        Poll::Ready(Some(Err(e))) => return Poll::Ready(Flow::Error(e)),
        Poll::Ready(None) => return Poll::Ready(Flow::Closed),
        Poll::Pending => {}
    }
    if let Some(reply) = pending_open.take() {
        match conn.poll_new_outbound(cx) {
            Poll::Ready(Ok(stream)) => {
                let _ = reply.send(Ok(Box::new(stream.compat())));
            }
            Poll::Ready(Err(e)) => {
                let _ = reply.send(Err(yamux_err(e)));
            }
            Poll::Pending => *pending_open = Some(reply),
        }
    }
    Poll::Pending
}

fn yamux_err(e: yamux::ConnectionError) -> io::Error {
    let kind = if matches!(e, yamux::ConnectionError::Closed) {
        io::ErrorKind::BrokenPipe
    } else {
        io::ErrorKind::ConnectionReset
    };
    io::Error::new(kind, e.to_string())
}

fn mux_closed() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "mux closed")
}
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn open_accept_roundtrip_between_two_muxes() {
        let (a, b) = tokio::io::duplex(64 * 1024);
        let mux_a = YamuxMux::new(Box::new(a), true, super::super::MAX_STREAMS_PER_CONN);
        let mux_b = YamuxMux::new(Box::new(b), false, super::super::MAX_STREAMS_PER_CONN);

        let mut stream_a = mux_a.open_stream().await.expect("open stream");
        stream_a.write_all(b"ping").await.expect("write ping");

        let mut stream_b = mux_b.accept_stream().await.expect("accept stream");
        let mut buf = [0u8; 4];
        stream_b.read_exact(&mut buf).await.expect("read ping");
        assert_eq!(&buf, b"ping");

        stream_b.write_all(b"pong").await.expect("write pong");
        let mut back = [0u8; 4];
        stream_a.read_exact(&mut back).await.expect("read pong");
        assert_eq!(&back, b"pong");
    }

    #[tokio::test]
    async fn accept_returns_none_after_peer_drops() {
        let (a, b) = tokio::io::duplex(64 * 1024);
        let mux_a = YamuxMux::new(Box::new(a), true, super::super::MAX_STREAMS_PER_CONN);
        let mux_b = YamuxMux::new(Box::new(b), false, super::super::MAX_STREAMS_PER_CONN);
        let mut stream_a = mux_a.open_stream().await.expect("open stream");
        stream_a.write_all(b"x").await.expect("write");
        drop(mux_a);
        drop(stream_a);
        assert!(mux_b.accept_stream().await.is_none(), "peer close must end accept");
    }
}