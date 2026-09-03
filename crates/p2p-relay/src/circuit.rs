//! 电路面：Connect 流配额检查、同号配对与 copy_bidirectional 密文桥接。
//! E6：桥接挂空闲监管——最近收发后双向静默满 idle_circuit_ttl 即拆桥回收；
//! 桥结束槽位立即退役，不再滞留至 reserve TTL 清扫（E5 备忘收口）。

use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use p2p_mux::BoxedStream;
use tokio::io::{copy_bidirectional, AsyncRead, AsyncWrite, ReadBuf};

use crate::frame::{write_msg, write_reject};
use crate::limits::RateLimitedStream;
use crate::messages::{errcode, RelayMsg};
use crate::service::RelayServiceImpl;
use crate::slots::{CircuitOutcome, PendingStream};

impl RelayServiceImpl {
    pub(crate) async fn handle_connect(
        self: Arc<Self>,
        joiner: String,
        stream: BoxedStream,
        cid: u64,
    ) {
        let outcome = {
            let mut st = self.lock_state();
            st.on_connect(&joiner, cid, self.limits().max_circuits_per_peer, stream)
        };
        match outcome {
            CircuitOutcome::Parked => {
                tracing::debug!(peer = %joiner, circuit = cid, "circuit half parked; waiting for peer");
            }
            CircuitOutcome::Paired(pending, stream) => {
                self.bridge(cid, pending, joiner, stream).await
            }
            CircuitOutcome::Rejected(code, message, mut stream) => {
                self.lock_state().metrics.count_connect_reject();
                tracing::warn!(peer = %joiner, circuit = cid, code, "connect rejected");
                let _ = write_reject(&mut stream, code, message).await;
            }
        }
    }

    /// 两侧都只是密文字节流：relay 不解析内容，限速按各自出口方向计数。
    async fn bridge(
        self: Arc<Self>,
        cid: u64,
        pending: PendingStream,
        joiner: String,
        mut stream: BoxedStream,
    ) {
        // 先向两侧各发 Bound（客户端 connect 依赖它返回），任一侧失败即取消桥接
        let mut parked = pending.stream;
        if let Err(e) = write_msg(&mut parked, &RelayMsg::bound(cid)).await {
            tracing::warn!(circuit = cid, holder = %pending.peer, error = %e, "bound write failed; bridge cancelled");
            let _ = write_reject(&mut stream, errcode::PROTOCOL, "circuit peer vanished").await;
            self.release_two(&pending.peer, &joiner);
            return;
        }
        if let Err(e) = write_msg(&mut stream, &RelayMsg::bound(cid)).await {
            tracing::warn!(circuit = cid, joiner = %joiner, error = %e, "bound write failed; bridge cancelled");
            self.release_two(&pending.peer, &joiner);
            return;
        }
        // 最近收发口径：桥两侧任意方向出现非空读或非空写即刷新时刻；仅双向
        // 持续静默满 idle_circuit_ttl 才判空闲（有数据流动的电路绝不被误收）。
        let base = Instant::now();
        let last = Arc::new(AtomicU64::new(0));
        let (last_a, last_sup) = (last.clone(), last.clone());
        let mut a = ActivityStream::new(
            RateLimitedStream::new(parked, self.bucket_for(&pending.peer)),
            base,
            last_a,
        );
        let mut b = ActivityStream::new(
            RateLimitedStream::new(stream, self.bucket_for(&joiner)),
            base,
            last,
        );
        let idle = self.keepalive().idle_circuit_ttl;
        let task = tokio::spawn(async move { copy_bidirectional(&mut a, &mut b).await });
        let (res, idle_dropped) = supervise_bridge(task, base, last_sup, idle).await;
        match res {
            Ok((a_to_b, b_to_a)) => {
                self.lock_state()
                    .metrics
                    .add_bridged_bytes(a_to_b.saturating_add(b_to_a));
                tracing::info!(circuit = cid, a_to_b, b_to_a, "circuit closed cleanly")
            }
            Err(_e) if idle_dropped => {
                self.lock_state().metrics.count_idle_reclaimed();
                tracing::warn!(
                    circuit = cid, a = %pending.peer, b = %joiner, idle_secs = idle.as_secs(),
                    "bridged circuit idle-reclaimed; slot retired and quota released"
                )
            }
            Err(e) => tracing::warn!(circuit = cid, error = %e, "circuit aborted"),
        }
        self.release_two(&pending.peer, &joiner);
        // 桥结束即退役槽位：属主配额回吐，槽不再滞留至 reserve TTL 清扫。
        // 先收口 MutexGuard 再 await（std MutexGuard 非 Send，不能跨 await）。
        let stranded = self.lock_state().retire_bridged_circuit(cid);
        if let Some(mut stranded) = stranded {
            tracing::warn!(circuit = cid, holder = %stranded.peer, "stranded post-bridge connect rejected");
            let _ = write_reject(
                &mut stranded.stream,
                errcode::CIRCUIT_EXPIRED,
                "circuit already retired",
            )
            .await;
        }
    }

    fn release_two(&self, peer_a: &str, peer_b: &str) {
        let mut st = self.lock_state();
        st.release_circuit_load(peer_a);
        st.release_circuit_load(peer_b);
    }
}

/// 桥接监管：桥任务完成即返回；否则每 idle 醒来查静默时长，超限拆桥回收。
/// 返回（copy 结果, 是否因空闲被回收）。idle 为零视为禁用监管。
async fn supervise_bridge(
    mut task: tokio::task::JoinHandle<io::Result<(u64, u64)>>,
    base: Instant,
    last: Arc<AtomicU64>,
    idle: Duration,
) -> (io::Result<(u64, u64)>, bool) {
    if idle.is_zero() {
        let res = task
            .await
            .unwrap_or_else(|e| Err(io::Error::other(e.to_string())));
        return (res, false);
    }
    loop {
        tokio::select! {
            res = &mut task => {
                let res = res.unwrap_or_else(|e| Err(io::Error::other(e.to_string())));
                return (res, false);
            }
            _ = tokio::time::sleep(idle) => {
                let idle_ms = Duration::from_millis(last.load(Ordering::Relaxed));
                let silent_for = base.elapsed().saturating_sub(idle_ms);
                if silent_for >= idle {
                    // [E6 消融点 1] 桥接空闲回收：删去 abort 与本分支即关闭该
                    // 逻辑（idle 回收用例必须随之变红，活跃反例仍绿）。
                    task.abort();
                    return (
                        Err(io::Error::new(io::ErrorKind::TimedOut, "bridged circuit idle")),
                        true,
                    );
                }
            }
        }
    }
}

/// 活动观测流：非空读写即刷新共享的最近收发时刻（毫秒，自 base 起）。
struct ActivityStream<S> {
    inner: S,
    base: Instant,
    last: Arc<AtomicU64>,
}

impl<S> ActivityStream<S> {
    fn new(inner: S, base: Instant, last: Arc<AtomicU64>) -> Self {
        Self { inner, base, last }
    }

    fn touch(&mut self) {
        let millis = u64::try_from(self.base.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.last.store(millis, Ordering::Relaxed);
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for ActivityStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let r = Pin::new(&mut this.inner).poll_read(cx, buf);
        if matches!(&r, Poll::Ready(Ok(())) if !buf.filled().is_empty()) {
            this.touch();
        }
        r
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for ActivityStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let r = Pin::new(&mut this.inner).poll_write(cx, buf);
        if matches!(&r, Poll::Ready(Ok(n)) if *n > 0) {
            this.touch();
        }
        r
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}
