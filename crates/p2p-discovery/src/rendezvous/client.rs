//! rendezvous 客户端：周期签名注册 + 查询 + last-known-good 缓存，事件推入统一 channel。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use p2p_identity::{Keypair, PeerId};
use p2p_transport::TransportAddr;
use tokio::sync::mpsc;

use crate::cache::MemCache;
use crate::rendezvous::link::{RendezvousConn, RendezvousError, RendezvousLink};
use crate::rendezvous::messages::{sign_register, unix_now, AddrMsg, Request, Response};
use crate::AddrCache;
use crate::{DiscoveredPeer, Discovery, DiscoveryEvent, Source};

/// 默认注册刷新间隔：须小于 QUIC 空闲超时（transport 侧 30s）兼作 keepalive，
/// 否则控制链路被掐断、注册出现间隙（E2/E3 实测）；同时远小于服务端
/// 新鲜度容差（±300s）与 TTL 截断（3600s）。
const DEFAULT_REGISTER_INTERVAL: Duration = Duration::from_secs(20);

/// 稳态查询间隔：全量轮询成本随池规模线性上涨（服务端每应答 O(N) 克隆+编码），
/// 默认 30s——快启动档之后即降到此档，配合抖动错开查询相位
/// （libp2p rendezvous 靠 cookie 增量，本实现先以降频控制全量轮询成本）。
const DEFAULT_QUERY_INTERVAL: Duration = Duration::from_secs(30);
/// 启动期快速查询间隔：上线后尽快填充邻居表。
const DEFAULT_QUERY_BOOT_INTERVAL: Duration = Duration::from_secs(5);
/// 快速档轮数：仅前 N 次重查用快速档，之后进入稳态档。
const DEFAULT_QUERY_BOOT_ROUNDS: u32 = 2;

/// 重连退避初值与上限：上限 30s 对齐服务端不可达时的探测节奏（E4 观测 ~35s 周期）。
const BACKOFF_INITIAL: Duration = Duration::from_millis(500);
const BACKOFF_MAX: Duration = Duration::from_secs(30);

/// 重连退避：失败逐次翻倍至上限；健康会话正常收尾即复位——
/// 退避只惩罚连续失败，长时间在线后一次断连不应等满上限（E4）。
pub(crate) struct ReconnectBackoff {
    initial: Duration,
    max: Duration,
    current: Duration,
}

impl ReconnectBackoff {
    pub(crate) fn new() -> Self {
        Self {
            initial: BACKOFF_INITIAL,
            max: BACKOFF_MAX,
            current: BACKOFF_INITIAL,
        }
    }

    /// 取本次等待时长并翻倍推进（封顶 max）。
    pub(crate) fn step(&mut self) -> Duration {
        let wait = self.current;
        self.current = (self.current * 2).min(self.max);
        wait
    }

    /// 健康会话收尾：退避复位到初值。
    pub(crate) fn reset(&mut self) {
        self.current = self.initial;
    }
}

/// rendezvous 客户端配置。
pub struct RendezvousConfig {
    pub namespace: String,
    pub keypair: Arc<Keypair>,
    /// 连接缝：真实传输由编排会话实现，测试用 duplex mock。
    pub link: Arc<dyn RendezvousLink>,
    /// 本机上报的监听地址。
    pub addrs: Vec<TransportAddr>,
    pub ttl_secs: u32,
    /// 注册刷新间隔，默认 20s（兼作控制链路 keepalive）。
    pub register_interval: Duration,
    /// 稳态查询间隔，默认 30s。
    pub query_interval: Duration,
    /// 启动期快速查询间隔，默认 5s（前 query_boot_rounds 轮生效）。
    pub query_boot_interval: Duration,
    /// 快速档轮数，默认 2；0 即全程稳态档。
    pub query_boot_rounds: u32,
    /// 查询侧地址卫生（E5）：剥离 loopback/link-local。信任域以 rendezvous 位置
    /// 为界——同机 rendezvous（bootstrap 全 loopback）保持 false，否则同机可发现性
    /// 会被误伤（a9be8e2 语义）。默认 true（跨网 rendezvous 是常态）。
    pub strip_unroutable: bool,
}

impl RendezvousConfig {
    pub fn new(
        namespace: impl Into<String>,
        keypair: Keypair,
        link: Arc<dyn RendezvousLink>,
    ) -> Self {
        Self {
            namespace: namespace.into(),
            keypair: Arc::new(keypair),
            link,
            addrs: Vec::new(),
            ttl_secs: 60,
            register_interval: DEFAULT_REGISTER_INTERVAL,
            query_interval: DEFAULT_QUERY_INTERVAL,
            query_boot_interval: DEFAULT_QUERY_BOOT_INTERVAL,
            query_boot_rounds: DEFAULT_QUERY_BOOT_ROUNDS,
            strip_unroutable: true,
        }
    }

    /// 第 query_round 次重查的等待时长：启动期走快速档，之后稳态档加 ±20% 抖动
    /// （错开全量节点的查询相位，防 bootstrap 应答风暴同步）。
    pub(crate) fn next_query_delay(&self, round: u32) -> Duration {
        if round < self.query_boot_rounds {
            return self.query_boot_interval;
        }
        use rand::Rng;
        let pct = rand::thread_rng().gen_range(0.8..1.2);
        Duration::from_secs_f64(self.query_interval.as_secs_f64() * pct)
    }
}

/// rendezvous 发现源：以独立任务运行，周期注册/查询，失败发 Failed 事件。
pub struct RendezvousClient {
    config: RendezvousConfig,
    /// 注册被拒首告警开关（E5）：同因反复失败只 WARN 一次，之后降级 debug。
    register_reject_warned: AtomicBool,
}

impl RendezvousClient {
    pub fn new(config: RendezvousConfig) -> Self {
        Self {
            config,
            register_reject_warned: AtomicBool::new(false),
        }
    }

    /// 签名注册本机地址；注册失败即让上层走 Failed 事件 + 重连。
    async fn register(&self, conn: &mut RendezvousConn) -> Result<(), RendezvousError> {
        let reg = sign_register(
            &self.config.keypair,
            &self.config.namespace,
            &self.config.addrs,
            self.config.ttl_secs,
            unix_now(),
        );
        let resp = conn.roundtrip(Request::register(reg)).await?;
        resp.ensure_ok().map_err(RendezvousError::Protocol)
    }

    /// 查询整个 namespace，把应答映射为 Discovered 事件并写入 last-known-good 缓存。
    async fn query_and_emit(
        &self,
        conn: &mut RendezvousConn,
        events: &mpsc::Sender<DiscoveryEvent>,
        cache: &MemCache,
    ) -> Result<(), RendezvousError> {
        let req = Request::query(self.config.namespace.clone(), Vec::new());
        let resp = conn.roundtrip(req).await?;
        resp.ensure_ok().map_err(RendezvousError::Protocol)?;
        let ttl = Duration::from_secs(self.config.ttl_secs.into());
        for (peer, addrs) in response_to_peers(&resp) {
            if peer == self.config.keypair.peer_id() {
                continue;
            }
            let addrs = if self.config.strip_unroutable {
                let Some(filtered) = routable_only(&addrs) else {
                    tracing::debug!(%peer, "rendezvous peer skipped: no routable addr");
                    continue;
                };
                filtered
            } else {
                addrs
            };
            cache.put(peer, addrs.clone(), ttl);
            let ev = DiscoveryEvent::Discovered(DiscoveredPeer {
                peer,
                addrs,
                source: Source::Rendezvous,
                expires_at: Some(Instant::now() + ttl),
            });
            if events.send(ev).await.is_err() {
                return Err(RendezvousError::Send("event channel closed".into()));
            }
        }
        Ok(())
    }

    /// 注册失败不阻断发现会话（E5 回归）：查询能力独立于本端注册
    /// （libp2p rendezvous 语义），无观测节点被拒后仍须能发现别人。
    /// 首次被拒 WARN 留痕，同因反复失败降级 debug；恢复成功即复位。
    async fn register_or_note(&self, conn: &mut RendezvousConn) {
        match self.register(conn).await {
            Ok(()) => self.register_reject_warned.store(false, Ordering::Relaxed),
            Err(e) => {
                let warned = self.register_reject_warned.swap(true, Ordering::Relaxed);
                if warned {
                    tracing::debug!(error = %e, "rendezvous register rejected; continuing query-only");
                } else {
                    tracing::warn!(error = %e, "rendezvous register rejected; continuing query-only");
                }
            }
        }
    }

    /// 一条连接上的注册/查询循环：断连即返回，由 run 重连。
    /// 查询节奏见 [`Self::next_query_delay`]：快启动后进入稳态降频。
    async fn connect_and_loop(
        &self,
        events: &mpsc::Sender<DiscoveryEvent>,
        cache: &MemCache,
    ) -> Result<(), RendezvousError> {
        let mut conn = self.config.link.connect().await?;
        self.register_or_note(&mut conn).await;
        self.query_and_emit(&mut conn, events, cache).await?;

        let reg_tick = tokio::time::interval_at(
            tokio::time::Instant::now() + self.config.register_interval,
            self.config.register_interval,
        );
        tokio::pin!(reg_tick);
        let mut query_round: u32 = 0;
        loop {
            tokio::select! {
                _ = reg_tick.tick() => self.register_or_note(&mut conn).await,
                _ = tokio::time::sleep(self.config.next_query_delay(query_round)) => {
                    self.query_and_emit(&mut conn, events, cache).await?;
                    query_round = query_round.saturating_add(1);
                }
            }
        }
    }
}

/// 查询侧地址卫生（E5）：丢弃 loopback/link-local 不可拨地址；全部不可路由
/// 返回 None（对端整体跳过，不产出「永远离线」条目污染邻居表）。私网保留。
pub(crate) fn routable_only(addrs: &[TransportAddr]) -> Option<Vec<TransportAddr>> {
    let out: Vec<TransportAddr> = addrs.iter().filter(|a| a.is_routable()).cloned().collect();
    (!out.is_empty()).then_some(out)
}

/// 把服务端应答解析为 (PeerId, 地址列表)；peer_id 非法或地址为空即跳过。
pub(crate) fn response_to_peers(resp: &Response) -> Vec<(PeerId, Vec<TransportAddr>)> {
    resp.peers
        .iter()
        .filter_map(|entry| {
            let peer_bytes = <[u8; 32]>::try_from(entry.peer_id.as_slice()).ok()?;
            let addrs: Vec<TransportAddr> =
                entry.addrs.iter().filter_map(AddrMsg::to_addr).collect();
            (!addrs.is_empty()).then_some((PeerId::from_bytes(peer_bytes), addrs))
        })
        .collect()
}

#[async_trait::async_trait]
impl Discovery for RendezvousClient {
    fn name(&self) -> &'static str {
        "rendezvous"
    }

    async fn run(self: Arc<Self>, events: mpsc::Sender<DiscoveryEvent>) {
        let cache = MemCache::new();
        let mut backoff = ReconnectBackoff::new();
        loop {
            match self.connect_and_loop(&events, &cache).await {
                Ok(()) => {
                    tracing::debug!(
                        target: "p2p_discovery",
                        "rendezvous connection ended, reconnecting"
                    );
                    backoff.reset();
                }
                Err(err) => {
                    let _ = events
                        .send(DiscoveryEvent::Failed {
                            source: Source::Rendezvous,
                            reason: err.to_string(),
                        })
                        .await;
                }
            }
            tokio::time::sleep(backoff.step()).await;
        }
    }
}

mod on_demand;

#[cfg(test)]
mod query_peer_tests;
#[cfg(test)]
mod schedule_tests;
#[cfg(test)]
mod tests;
