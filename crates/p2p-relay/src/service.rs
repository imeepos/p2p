//! RelayService 实现：收链路、首帧分流（Reserve=控制 / Connect=电路）、桥接密文字节。
//!
//! relay 全程只搬运字节，不解不存（design 7.3）；每 Peer 配额见 limits 模块。

use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use p2p_mux::BoxedStream;

use crate::frame::{read_msg, write_reject};
use crate::keepalive::RelayKeepalive;
use crate::limits::{PeerBuckets, RateBucket, RelayLimits};
use crate::link::{LinkSource, RelayLink};
use crate::messages::{errcode, relay_msg::Kind};
use crate::state::RelayState;
use crate::RelayService;

/// 到期电路回收周期。
const SWEEP_INTERVAL: Duration = Duration::from_secs(1);
/// 超限链路拒绝循环的有界参数（审查 M2）：最多回 64 条拒绝帧、空闲 10s 即撤。
const REJECT_MAX_STREAMS: u32 = 64;
const REJECT_IDLE: Duration = Duration::from_secs(10);

/// 服务端：对 LinkSource 接缝编程，不依赖具体 transport。
pub struct RelayServiceImpl {
    source: Box<dyn LinkSource>,
    limits: RelayLimits,
    keepalive: RelayKeepalive,
    buckets: PeerBuckets,
    state: Mutex<RelayState>,
    /// 控制流代次发生器：每条新控制流一个唯一代次，电路记账据此归属。
    ctrl_epoch: AtomicU64,
}

impl RelayServiceImpl {
    pub fn new(source: Box<dyn LinkSource>, limits: RelayLimits) -> Self {
        Self::with_keepalive(source, limits, RelayKeepalive::default())
    }

    /// 指定保活/静默/空闲回收参数的服务端（E6）；默认值依据见 RelayKeepalive。
    pub fn with_keepalive(
        source: Box<dyn LinkSource>,
        limits: RelayLimits,
        keepalive: RelayKeepalive,
    ) -> Self {
        Self {
            source,
            buckets: PeerBuckets::new(limits.clone()),
            limits,
            keepalive,
            state: Mutex::new(RelayState::new()),
            ctrl_epoch: AtomicU64::new(1),
        }
    }

    /// 下一个控制流代次（0 保留给测试桩）。
    pub(crate) fn next_ctrl_epoch(&self) -> u64 {
        self.ctrl_epoch.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn limits(&self) -> &RelayLimits {
        &self.limits
    }

    pub(crate) fn keepalive(&self) -> &RelayKeepalive {
        &self.keepalive
    }

    /// 中毒恢复而非 panic（审查 L3）：临界区均为单段无 await 的短操作，
    /// 取回内部状态继续服务，代价可控；中毒本身留 error 日志。
    pub(crate) fn lock_state(&self) -> std::sync::MutexGuard<'_, RelayState> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::error!("relay state mutex poisoned; recovering inner state");
                poisoned.into_inner()
            }
        }
    }

    pub(crate) fn bucket_for(&self, peer: &str) -> Arc<Mutex<RateBucket>> {
        self.buckets.bucket_for(peer)
    }

    /// 服务端指标快照（E5）：电路/链路水位与发放/拒绝累计。
    pub fn metrics(&self) -> crate::metrics::RelayMetricsSnapshot {
        let state = self.lock_state();
        state.metrics.snapshot(&state)
    }
}

#[async_trait]
impl RelayService for RelayServiceImpl {
    async fn serve(self: Arc<Self>) -> io::Result<()> {
        let sweeper = tokio::spawn(self.clone().sweep_loop());
        while let Some(link) = self.source.next_link().await {
            tracing::debug!(peer = %link.peer_id(), "relay accepted link");
            tokio::spawn(self.clone().handle_link(link));
        }
        tracing::info!("relay link source closed; accept loop stopped");
        sweeper.abort();
        Ok(())
    }
}

impl RelayServiceImpl {
    async fn sweep_loop(self: Arc<Self>) {
        let mut tick = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            tick.tick().await;
            let gone = {
                let mut state = self.lock_state();
                let gone = state.sweep_expired(Instant::now());
                if !gone.is_empty() {
                    state.metrics.count_expired(gone.len() as u64);
                }
                gone
            };
            for g in gone {
                tracing::warn!(circuit = g.cid, holder = ?g.holder, "circuit reservation expired; dropped");
            }
        }
    }

    async fn handle_link(self: Arc<Self>, link: Box<dyn RelayLink>) {
        let peer = link.peer_id().to_string();
        // 锁临界区先收口再 await：std MutexGuard 非 Send，不能跨 await 持有
        let admitted = self.lock_state().register_link(
            &peer,
            self.limits.max_links_per_peer,
            self.limits.max_total_links,
        );
        if let Err(reason) = admitted {
            self.lock_state().metrics.count_link_reject();
            tracing::warn!(peer = %peer, reason = ?reason, "link rejected: quota");
            self.reject_link(link).await;
            return;
        }
        while let Some(stream) = link.accept_stream().await {
            tokio::spawn(self.clone().handle_stream(peer.clone(), stream));
        }
        tracing::info!(peer = %peer, "peer link closed");
        let went_zero = self.lock_state().unregister_link(&peer);
        if went_zero {
            // 对端整体消失：未桥接电路随链路归零回收（流级 EOF 可能传播不到）
            self.release_all_circuits_of_peer(&peer).await;
        }
        if self.lock_state().peer_idle(&peer) && self.buckets.release(&peer) {
            tracing::debug!(peer = %peer, "idle peer bucket reclaimed");
        }
    }

    /// 超限链路：对每条流回显式拒绝帧后关闭，给客户端确定性信号。
    /// 有界（审查 M2）：最多 REJECT_MAX_STREAMS 条、空闲 REJECT_IDLE 即放弃，
    /// 防恶意连接以「持续发流」钉住本任务与文件描述符。
    async fn reject_link(&self, link: Box<dyn RelayLink>) {
        let mut answered = 0u32;
        loop {
            if answered >= REJECT_MAX_STREAMS {
                tracing::warn!("reject link stream cap reached; dropping link");
                return;
            }
            let next = tokio::time::timeout(REJECT_IDLE, link.accept_stream()).await;
            match next {
                Ok(Some(mut stream)) => {
                    answered += 1;
                    if let Err(e) = write_reject(
                        &mut stream,
                        errcode::PEER_LIMIT,
                        "per-peer link quota exceeded",
                    )
                    .await
                    {
                        tracing::warn!(error = %e, "reject frame write failed on over-quota link");
                        return;
                    }
                }
                Ok(None) => return,
                Err(_) => {
                    tracing::debug!("reject link idle timeout; dropping link");
                    return;
                }
            }
        }
    }

    /// 首帧分流：Reserve 升格控制流，Connect 进电路面，其余按协议违规拒绝。
    async fn handle_stream(self: Arc<Self>, peer: String, mut stream: BoxedStream) {
        let first = match read_msg(&mut stream).await {
            Ok(Some(msg)) => msg,
            Ok(None) => return,
            Err(e) => {
                tracing::warn!(peer = %peer, error = %e, "bad first frame; dropping stream");
                return;
            }
        };
        match first.kind {
            Some(Kind::Reserve(r)) => self.handle_control(peer, stream, r).await,
            Some(Kind::Connect(c)) => self.handle_connect(peer, stream, c.circuit_id).await,
            other => {
                tracing::warn!(peer = %peer, kind = ?other, "protocol violation: unexpected first frame");
                let _ =
                    write_reject(&mut stream, errcode::PROTOCOL, "unexpected first frame").await;
            }
        }
    }
}
