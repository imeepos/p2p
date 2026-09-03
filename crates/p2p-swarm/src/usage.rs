//! 连接使用度记账（E8 调研建议 4）：空闲回收的「使用中」判据。
//!
//! 使用中的定义：有在途业务流，或有近期的业务开流/收流。探活 ping 是
//! 底座自带的维持流量，不计入使用——否则生命周期监督者的周期探测会让
//! 池内连接永不空闲，空闲回收形同虚设；不被业务使用的连接本就该回收，
//! 需要时 connect() 幂等重拨即可。

use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use p2p_mux::BoxedStream;

/// UNIX 秒（秒级粒度即本轮统一口径）；时钟回拨退化为 0，仅影响一次阈值比较。
pub(crate) fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 单条连接的使用记账：池条目与外发的流守护共享同一 Arc，
/// 连接被顶替/回收后旧流归还计数到孤儿 Arc 上，无害。
#[derive(Default)]
pub(crate) struct ConnUsage {
    in_flight: AtomicUsize,
    last_used_unix: AtomicU64,
}

impl ConnUsage {
    pub(crate) fn new(now_unix: u64) -> Self {
        Self {
            in_flight: AtomicUsize::new(0),
            last_used_unix: AtomicU64::new(now_unix),
        }
    }

    /// 业务活跃：刷新最后使用时刻（开流成功/收流到达时调用）。
    pub(crate) fn touch(&self, now_unix: u64) {
        self.last_used_unix.store(now_unix, Ordering::Relaxed);
    }

    /// 进入在途（开流前取守护，失败即析构归还，净零）。
    pub(crate) fn enter(self: &Arc<Self>) -> InflightGuard {
        self.in_flight.fetch_add(1, Ordering::Relaxed);
        InflightGuard {
            usage: self.clone(),
        }
    }

    fn exit(&self) {
        self.in_flight.fetch_sub(1, Ordering::Relaxed);
    }

    /// 空闲判据：无在途流且距最后使用超过阈值。消融锚点：
    /// 撤掉 in_flight 判空即豁免失效，itest 在途豁免用例必红。
    pub(crate) fn is_idle(&self, threshold: Duration, now_unix: u64) -> bool {
        self.in_flight.load(Ordering::Relaxed) == 0
            && now_unix.saturating_sub(self.last_used_unix.load(Ordering::Relaxed))
                >= threshold.as_secs()
    }
}

/// 在途流守护：析构时归还计数，流（含错误路径）半途丢弃不漏计。
pub(crate) struct InflightGuard {
    usage: Arc<ConnUsage>,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.usage.exit();
    }
}

/// 业务流的记账包装：读写字节委托内层，仅借 Drop 维护在途计数。
pub(crate) struct TrackedStream {
    inner: BoxedStream,
    _guard: InflightGuard,
}

impl TrackedStream {
    pub(crate) fn new(inner: BoxedStream, guard: InflightGuard) -> Self {
        Self {
            inner,
            _guard: guard,
        }
    }
}

impl tokio::io::AsyncRead for TrackedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for TrackedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

// ByteStream 由 p2p_mux 的 blanket impl（AsyncRead+AsyncWrite+Unpin+Send）自动满足。

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_requires_zero_in_flight_and_aged_last_use() {
        let usage = Arc::new(ConnUsage::new(1000));
        // 5s 阈值：1000s 时刻已空闲但 guard 在途即豁免
        let guard = usage.enter();
        assert!(!usage.is_idle(Duration::from_secs(5), 1300));
        drop(guard);
        assert!(usage.is_idle(Duration::from_secs(5), 1300));
        // 有近期使用：touch 后回退到未空闲
        usage.touch(1400);
        assert!(!usage.is_idle(Duration::from_secs(5), 1402));
        assert!(usage.is_idle(Duration::from_secs(5), 1406));
    }
}
