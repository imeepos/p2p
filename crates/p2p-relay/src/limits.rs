//! 防滥用红线（design 6/14）：每 Peer 连接数/电路数/出口带宽上限。
//!
//! 带宽用惰性令牌桶：桥接流出口写前扣令牌，不足即 WriteZero，
//! copy_bidirectional 随即断链并留日志（超额断链，不做无限期节流）。

use std::collections::HashMap;
use std::io::{self, Error, ErrorKind};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Instant;

use p2p_mux::BoxedStream;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// relay 服务端配额；Default 为公网节点的保守值。
#[derive(Debug, Clone)]
pub struct RelayLimits {
    /// 单 Peer 同时挂入的链路数上限。
    pub max_links_per_peer: usize,
    /// 单 Peer 同时持有的电路配额（owner 与接入方分别计）。
    pub max_circuits_per_peer: usize,
    /// 每 Peer 出口令牌补充速率（字节/秒）。
    pub egress_bytes_per_sec: u64,
    /// 每 Peer 出口令牌桶容量（突发余量，字节）。
    pub egress_burst: u64,
}

impl Default for RelayLimits {
    fn default() -> Self {
        Self {
            max_links_per_peer: 8,
            max_circuits_per_peer: 32,
            egress_bytes_per_sec: 1 << 20,
            egress_burst: 1 << 20,
        }
    }
}

/// 惰性补充令牌桶。
pub struct RateBucket {
    capacity: f64,
    refill_per_sec: f64,
    tokens: f64,
    last: Instant,
}

impl RateBucket {
    pub fn new(capacity: u64, refill_per_sec: u64) -> Self {
        Self {
            capacity: capacity as f64,
            refill_per_sec: refill_per_sec as f64,
            tokens: capacity as f64,
            last: Instant::now(),
        }
    }

    /// 扣 n 个令牌；不足（含 n 超容量）返回 false，由调用方断链。
    pub fn try_take(&mut self, n: u64) -> bool {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * self.refill_per_sec).min(self.capacity);
        if self.tokens >= n as f64 {
            self.tokens -= n as f64;
            true
        } else {
            false
        }
    }
}

/// 按 Peer 惰性建桶；同一 Peer 的所有出口流共享一个桶。
#[derive(Clone)]
pub struct PeerBuckets {
    map: Arc<Mutex<HashMap<String, Arc<Mutex<RateBucket>>>>>,
    limits: RelayLimits,
}

impl PeerBuckets {
    pub fn new(limits: RelayLimits) -> Self {
        Self { map: Arc::new(Mutex::new(HashMap::new())), limits }
    }

    pub fn bucket_for(&self, peer: &str) -> Arc<Mutex<RateBucket>> {
        let mut map = self.map.lock().expect("peer buckets poisoned");
        map.entry(peer.to_string())
            .or_insert_with(|| {
                Arc::new(Mutex::new(RateBucket::new(
                    self.limits.egress_burst,
                    self.limits.egress_bytes_per_sec,
                )))
            })
            .clone()
    }
}

/// 出口限速流：读直通，写扣令牌，超额返回 WriteZero。
pub struct RateLimitedStream {
    inner: BoxedStream,
    bucket: Arc<Mutex<RateBucket>>,
}

impl RateLimitedStream {
    pub fn new(inner: BoxedStream, bucket: Arc<Mutex<RateBucket>>) -> Self {
        Self { inner, bucket }
    }
}

impl AsyncRead for RateLimitedStream {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for RateLimitedStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let allowed = {
            let mut bucket = this.bucket.lock().expect("rate bucket poisoned");
            bucket.try_take(buf.len() as u64)
        };
        if !allowed {
            return Poll::Ready(Err(Error::new(ErrorKind::WriteZero, "relay egress quota exceeded")));
        }
        Pin::new(&mut this.inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn bucket_refuses_beyond_capacity() {
        let mut b = RateBucket::new(1000, 0);
        assert!(b.try_take(600));
        assert!(!b.try_take(600));
    }

    #[tokio::test]
    async fn bucket_refills_over_time() {
        let mut b = RateBucket::new(1000, 200_000);
        assert!(b.try_take(1000));
        assert!(!b.try_take(1000));
        // 30ms 补约 6000，封顶回满
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(b.try_take(1000));
    }

    #[tokio::test]
    async fn rate_limited_write_fails_when_exhausted() {
        let (tx, rx) = tokio::io::duplex(4096);
        let bucket = Arc::new(Mutex::new(RateBucket::new(512, 512)));
        let mut limited = RateLimitedStream::new(Box::new(tx), bucket);
        limited.write_all(&[0u8; 256]).await.expect("within burst");
        limited.flush().await.unwrap();
        let err = limited.write_all(&[0u8; 1024]).await.expect_err("beyond burst");
        assert_eq!(err.kind(), ErrorKind::WriteZero);
        drop(rx);
    }

    #[tokio::test]
    async fn rate_limited_read_passes_through() {
        let (tx, rx) = tokio::io::duplex(1024);
        let bucket = Arc::new(Mutex::new(RateBucket::new(1 << 20, 1 << 20)));
        tokio::spawn(async move {
            let mut t = tx;
            t.write_all(b"hello").await.unwrap();
            t.flush().await.unwrap();
        });
        let mut limited = RateLimitedStream::new(Box::new(rx), bucket);
        let mut buf = [0u8; 5];
        limited.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello");
    }
}
