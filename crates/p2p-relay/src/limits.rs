//! 防滥用红线（design 6/14）：每 Peer 连接数/电路数/出口带宽上限，
//! 外加全站总量上限（审查 M5：每 Peer 粒度可被 Sybil 身份稀释）。
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
    /// 全站链路总量上限（审查 M5）。
    pub max_total_links: usize,
    /// 全站存活电路（reservation）总量上限（审查 M5）。
    pub max_total_circuits: usize,
    /// 全站带宽桶数量上限；打满后新 Peer 共享降级桶并告警（审查 M5）。
    pub max_total_buckets: usize,
    /// 单控制流打洞信令速率上限（条/分钟），令牌桶（审查 M3）。
    pub max_punch_per_minute: u32,
}

impl Default for RelayLimits {
    fn default() -> Self {
        Self {
            max_links_per_peer: 8,
            max_circuits_per_peer: 32,
            egress_bytes_per_sec: 1 << 20,
            egress_burst: 1 << 20,
            max_total_links: 256,
            max_total_circuits: 1024,
            max_total_buckets: 256,
            max_punch_per_minute: 60,
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
    /// 桶表打满时的共享降级桶（保连通性，限内存）。
    fallback: Arc<Mutex<RateBucket>>,
}

impl PeerBuckets {
    pub fn new(limits: RelayLimits) -> Self {
        let fallback = Arc::new(Mutex::new(RateBucket::new(
            limits.egress_burst,
            limits.egress_bytes_per_sec,
        )));
        Self {
            map: Arc::new(Mutex::new(HashMap::new())),
            limits,
            fallback,
        }
    }

    pub fn bucket_for(&self, peer: &str) -> Arc<Mutex<RateBucket>> {
        let mut map = self.map.lock().expect("peer buckets poisoned");
        if !map.contains_key(peer) && map.len() >= self.limits.max_total_buckets {
            tracing::warn!(
                peer = %peer,
                cap = self.limits.max_total_buckets,
                "bucket table full; sharing fallback bucket"
            );
            return self.fallback.clone();
        }
        map.entry(peer.to_string())
            .or_insert_with(|| {
                Arc::new(Mutex::new(RateBucket::new(
                    self.limits.egress_burst,
                    self.limits.egress_bytes_per_sec,
                )))
            })
            .clone()
    }

    /// Peer 闲置（链路与在途电路流清零）后回收其桶，防表只增不减（审查 M5）。
    pub fn release(&self, peer: &str) -> bool {
        self.map
            .lock()
            .expect("peer buckets poisoned")
            .remove(peer)
            .is_some()
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
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for RateLimitedStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let allowed = {
            let mut bucket = this.bucket.lock().expect("rate bucket poisoned");
            bucket.try_take(buf.len() as u64)
        };
        if !allowed {
            return Poll::Ready(Err(Error::new(
                ErrorKind::WriteZero,
                "relay egress quota exceeded",
            )));
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
        let err = limited
            .write_all(&[0u8; 1024])
            .await
            .expect_err("beyond burst");
        assert_eq!(err.kind(), ErrorKind::WriteZero);
        drop(rx);
    }

    #[test]
    fn bucket_reclaimed_on_release() {
        let pb = PeerBuckets::new(RelayLimits::default());
        let b1 = pb.bucket_for("p");
        assert!(pb.release("p"));
        assert!(!pb.release("p"));
        // 旧桶已回收：新会话拿到全新桶，不继承旧令牌
        let b2 = pb.bucket_for("p");
        assert!(!Arc::ptr_eq(&b1, &b2));
    }

    #[test]
    fn bucket_table_cap_falls_back_to_shared() {
        let limits = RelayLimits {
            max_total_buckets: 1,
            ..RelayLimits::default()
        };
        let pb = PeerBuckets::new(limits);
        let b1 = pb.bucket_for("p1");
        let b2 = pb.bucket_for("p2");
        assert!(Arc::ptr_eq(&b2, &pb.fallback), "溢出 Peer 共享降级桶");
        assert!(!Arc::ptr_eq(&b1, &b2));
        assert!(pb.release("p1"));
        assert!(!pb.release("p2"), "降级桶不在表中，无可回收项");
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
