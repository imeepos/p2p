//! RelayService 实现：收链路、首帧分流（Reserve=控制 / Connect=电路）、桥接密文字节。
//!
//! relay 全程只搬运字节，不解不存（design 7.3）；每 Peer 配额见 limits 模块。

use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use p2p_mux::BoxedStream;

use crate::frame::{read_msg, write_reject};
use crate::limits::{PeerBuckets, RateBucket, RelayLimits};
use crate::link::{LinkSource, RelayLink};
use crate::messages::{errcode, relay_msg::Kind};
use crate::state::RelayState;
use crate::RelayService;

/// 到期电路回收周期。
const SWEEP_INTERVAL: Duration = Duration::from_secs(1);

/// 服务端：对 LinkSource 接缝编程，不依赖具体 transport。
pub struct RelayServiceImpl {
    source: Box<dyn LinkSource>,
    limits: RelayLimits,
    buckets: PeerBuckets,
    state: Mutex<RelayState>,
}

impl RelayServiceImpl {
    pub fn new(source: Box<dyn LinkSource>, limits: RelayLimits) -> Self {
        Self {
            source,
            buckets: PeerBuckets::new(limits.clone()),
            limits,
            state: Mutex::new(RelayState::new()),
        }
    }

    pub(crate) fn limits(&self) -> &RelayLimits {
        &self.limits
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
            for gone in self.lock_state().sweep_expired(Instant::now()) {
                tracing::warn!(circuit = gone.cid, holder = ?gone.holder, "circuit reservation expired; dropped");
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
            tracing::warn!(peer = %peer, reason = ?reason, "link rejected: quota");
            self.reject_link(link).await;
            return;
        }
        while let Some(stream) = link.accept_stream().await {
            tokio::spawn(self.clone().handle_stream(peer.clone(), stream));
        }
        tracing::info!(peer = %peer, "peer link closed");
        self.lock_state().unregister_link(&peer);
        if self.lock_state().peer_idle(&peer) && self.buckets.release(&peer) {
            tracing::debug!(peer = %peer, "idle peer bucket reclaimed");
        }
    }

    /// 超限链路：对每条流回显式拒绝帧后关闭，给客户端确定性信号。
    async fn reject_link(&self, link: Box<dyn RelayLink>) {
        while let Some(mut stream) = link.accept_stream().await {
            if let Err(e) = write_reject(
                &mut stream,
                errcode::PEER_LIMIT,
                "per-peer link quota exceeded",
            )
            .await
            {
                tracing::warn!(error = %e, "reject frame write failed on over-quota link");
                break;
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
