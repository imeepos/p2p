//! Noise 传输态的 tokio 流封装：[u16 帧长][密文] 分帧，逐帧解密。
//!
//! 上层读写透明；单帧明文上限 [PLAIN_CHUNK]，避免触及 Noise 65535 报文上限。

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use snow::TransportState;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// 每帧明文上限：8 KiB，串成流后对上层无感
pub(crate) const PLAIN_CHUNK: usize = 8 * 1024;
const TAG_LEN: usize = 16;

enum RxState {
    /// 读 2 字节帧长
    Len { buf: [u8; 2], got: usize },
    /// 读帧体并解密
    Body {
        len: usize,
        buf: Vec<u8>,
        pos: usize,
    },
}

pub(crate) struct NoiseStream<S> {
    io: S,
    state: TransportState,
    rx: RxState,
    /// 已解密待读明文
    plain: Vec<u8>,
    plain_pos: usize,
    /// 已加密待写出密文
    out: Vec<u8>,
}

impl<S> NoiseStream<S> {
    pub(crate) fn new(io: S, state: TransportState) -> Self {
        Self {
            io,
            state,
            rx: RxState::Len {
                buf: [0u8; 2],
                got: 0,
            },
            plain: Vec::new(),
            plain_pos: 0,
            out: Vec::new(),
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> NoiseStream<S> {
    fn flush_out(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        while self.plain_pos < self.out.len() || !self.out.is_empty() {
            let n = match Pin::new(&mut self.io).poll_write(cx, &self.out) {
                Poll::Ready(Ok(n)) => n,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            };
            if n == 0 {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "noise stream: inner write returned 0",
                )));
            }
            self.out.drain(..n);
        }
        Poll::Ready(Ok(()))
    }

    /// 推进接收状态机；有完整帧即解密追加到 plain
    fn advance_rx(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            match &mut self.rx {
                RxState::Len { buf, got } => {
                    while *got < 2 {
                        let mut rb = ReadBuf::new(&mut buf[*got..]);
                        match Pin::new(&mut self.io).poll_read(cx, &mut rb) {
                            Poll::Ready(Ok(())) => {}
                            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                            Poll::Pending => return Poll::Pending,
                        }
                        if rb.filled().is_empty() {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "noise stream: closed mid frame",
                            )));
                        }
                        *got += rb.filled().len();
                    }
                    let len = u16::from_be_bytes([buf[0], buf[1]]) as usize;
                    self.rx = RxState::Body {
                        len,
                        buf: vec![0u8; len],
                        pos: 0,
                    };
                }
                RxState::Body { len, buf, pos } => {
                    while *pos < *len {
                        let mut rb = ReadBuf::new(&mut buf[*pos..]);
                        match Pin::new(&mut self.io).poll_read(cx, &mut rb) {
                            Poll::Ready(Ok(())) => {}
                            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                            Poll::Pending => return Poll::Pending,
                        }
                        if rb.filled().is_empty() {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "noise stream: closed mid body",
                            )));
                        }
                        *pos += rb.filled().len();
                    }
                    let mut plain = vec![0u8; *len];
                    let n = self.state.read_message(buf, &mut plain).map_err(|e| {
                        io::Error::new(io::ErrorKind::InvalidData, format!("noise decrypt: {e}"))
                    })?;
                    plain.truncate(n);
                    self.plain = plain;
                    self.plain_pos = 0;
                    self.rx = RxState::Len {
                        buf: [0u8; 2],
                        got: 0,
                    };
                    return Poll::Ready(Ok(()));
                }
            }
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncRead for NoiseStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        loop {
            if this.plain_pos < this.plain.len() {
                let n = (buf.remaining()).min(this.plain.len() - this.plain_pos);
                buf.put_slice(&this.plain[this.plain_pos..this.plain_pos + n]);
                this.plain_pos += n;
                return Poll::Ready(Ok(()));
            }
            match this.advance_rx(cx) {
                Poll::Ready(Ok(())) => {} // 一帧解密完毕，回到循环顶部供读
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncWrite for NoiseStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        // 先冲刷上一轮密文，保证写序
        match this.flush_out(cx) {
            Poll::Ready(Ok(())) => {}
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Pending => return Poll::Pending,
        }
        let chunk_len = buf.len().min(PLAIN_CHUNK);
        let mut frame = Vec::with_capacity(chunk_len + TAG_LEN + 2);
        frame.extend_from_slice(&(chunk_len as u16 + TAG_LEN as u16).to_be_bytes());
        let mut ct = vec![0u8; chunk_len + TAG_LEN];
        let n = this
            .state
            .write_message(&buf[..chunk_len], &mut ct)
            .map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("noise encrypt: {e}"))
            })?;
        frame.extend_from_slice(&ct[..n]);
        this.out = frame;
        Poll::Ready(Ok(chunk_len))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.get_mut().flush_out(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match this.flush_out(cx) {
            Poll::Ready(Ok(())) => {}
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Pending => return Poll::Pending,
        }
        Pin::new(&mut this.io).poll_shutdown(cx)
    }
}
