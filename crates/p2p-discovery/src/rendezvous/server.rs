//! rendezvous 服务端：按 namespace 维护带 TTL 的注册表，校验签名后应答查询。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use prost::Message as _;

use p2p_identity::PeerId;
use p2p_transport::TransportAddr;

use crate::cache::MemCache;
use crate::rendezvous::link::{RendezvousConn, RendezvousError};
use crate::rendezvous::messages::{
    request, unix_now, verify_register, AddrMsg, PeerEntry, Query, Register, Request, Response,
};
use crate::AddrCache;

mod snapshot;

/// namespace 名称长度上限（字节），防恶意超长键撑内存（M1）。
pub const MAX_NAMESPACE_LEN: usize = 64;
/// 单个 namespace 的 peer 数上限（M1）。
pub const MAX_PEERS_PER_NAMESPACE: usize = 512;
/// 单条注册 TTL 上限（秒），防 136 年长占（H1，对齐 relay MAX_TTL_SECS）。
pub const MAX_TTL_SECS: u64 = 3600;
/// 每连接注册频率上限（次/分），令牌桶（M1）。
pub const RATE_LIMIT_PER_MINUTE: u32 = 10;
/// 每连接查询频率上限（次/分），令牌桶（审查 M8）。
pub const RATE_LIMIT_QUERIES_PER_MINUTE: u32 = 120;
/// 单条注册地址数上限（审查 M8），防大注册帧撑验签与存储。
pub const MAX_ADDRS_PER_REGISTER: usize = 32;

/// rendezvous 注册表：namespace → 带 TTL 的地址缓存。
/// public_only：公共部署策略，拒收全 loopback/link-local 注册（E5 地址卫生）。
/// 签名记录不可改写，故只做整单拒收，不剥离部分地址；默认宽松（同机/单测场景）。
#[derive(Default)]
pub struct RendezvousRegistry {
    namespaces: Mutex<HashMap<String, MemCache>>,
    /// 全量查询应答快照（snapshot.rs）：削峰 O(N) 编码，register 失效、TTL 到期重建。
    snapshots: snapshot::SnapshotStore,
    public_only: bool,
}

impl RendezvousRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_public_only(public_only: bool) -> Self {
        Self {
            public_only,
            ..Default::default()
        }
    }

    /// 处理注册：namespace/签名新鲜度/资源上限校验通过后入库；失败返回 Err 并留日志。
    pub fn register(&self, reg: &Register, now: u64) -> Result<(), String> {
        if reg.namespace.is_empty() {
            return Err("empty namespace".to_string());
        }
        if reg.namespace.len() > MAX_NAMESPACE_LEN {
            return Err("namespace too long".to_string());
        }
        if reg.addrs.len() > MAX_ADDRS_PER_REGISTER {
            // 仅限上限；空地址注册为存量兼容语义，保留（无发现价值但无害）
            return Err(format!("addr count above {MAX_ADDRS_PER_REGISTER}"));
        }
        if !verify_register(reg, now) {
            tracing::warn!(
                target: "p2p_discovery",
                "rendezvous register rejected: bad signature or stale, peer {:?}",
                reg.peer_id
            );
            return Err("bad signature or stale register".to_string());
        }
        let peer: [u8; 32] = reg
            .peer_id
            .as_slice()
            .try_into()
            .map_err(|_| "malformed peer_id".to_string())?;
        let peer = PeerId::from_bytes(peer);
        // verify_register 已保证全部地址可解析且非空，此处直接收集
        let addrs: Vec<TransportAddr> = reg.addrs.iter().filter_map(AddrMsg::to_addr).collect();
        let ttl = Duration::from_secs(u64::from(reg.ttl_secs).min(MAX_TTL_SECS));
        if self.public_only && !addrs.is_empty() && !addrs.iter().any(TransportAddr::is_routable) {
            tracing::warn!(
                target: "p2p_discovery",
                "rendezvous register rejected: no routable addr, peer {peer:?}"
            );
            return Err("no routable addr".to_string());
        }
        let mut map = self.namespaces.lock().unwrap_or_else(|p| p.into_inner());
        let cache = map.entry(reg.namespace.clone()).or_default();
        // 每 namespace peer 数上限：仅新增且已满才拒，覆盖已有 peer 不受限
        if cache.get(&peer).is_none() && cache.snapshot().len() >= MAX_PEERS_PER_NAMESPACE {
            return Err("namespace peer limit reached".to_string());
        }
        cache.put(peer, addrs, ttl);
        self.snapshots.invalidate(&reg.namespace);
        Ok(())
    }

    /// 应答查询：peer_id 为空返回整个 namespace 的未过期条目。
    /// 只读不扩表（审计 HIGH）：未知/非法 namespace 返回空结果，绝不创建缓存条目，
    /// 否则任意连接可用随机 namespace 查询撑大服务端内存。
    pub fn query(&self, q: &Query) -> Response {
        let empty = Response {
            error: String::new(),
            peers: Vec::new(),
        };
        if q.namespace.is_empty() || q.namespace.len() > MAX_NAMESPACE_LEN {
            return empty;
        }
        let target: Option<PeerId> = q.peer_id.as_slice().try_into().ok().map(PeerId::from_bytes);
        let map = self.namespaces.lock().unwrap_or_else(|p| p.into_inner());
        let Some(cache) = map.get(&q.namespace) else {
            return empty;
        };
        let found = match target {
            // 单 peer 精确查询走键读取（社交化发现 P1：查号台 O(1)），
            // 不做全表克隆；过期条目同样即时清除
            Some(t) => cache
                .get(&t)
                .map(|addrs| vec![(t, addrs)])
                .unwrap_or_default(),
            None => cache.snapshot(),
        };
        let peers = found
            .into_iter()
            .map(|(peer, addrs)| PeerEntry {
                peer_id: peer.as_bytes().to_vec(),
                addrs: addrs.iter().map(AddrMsg::from_addr).collect(),
            })
            .collect();
        Response {
            error: String::new(),
            peers,
        }
    }

    /// 应答查询并返回编码帧：全量查询走快照缓存，单 peer 精确查询按需
    /// 编码不进缓存（结果小且访问模式随机，缓存无收益）。
    pub fn query_encoded(&self, q: &Query) -> Vec<u8> {
        if q.peer_id.is_empty() && !q.namespace.is_empty() && q.namespace.len() <= MAX_NAMESPACE_LEN
        {
            return self.full_query_encoded(q);
        }
        encode_response(&self.query(q))
    }

    /// 全量查询：命中快照直接回；未命中在 namespaces 锁内重建并记录，
    /// 有效窗口取条目真实最早过期时刻（register 与重建经同一把锁串行，
    /// 不存在「先插入陈旧快照后失效」的交错）。
    fn full_query_encoded(&self, q: &Query) -> Vec<u8> {
        let now = Instant::now();
        if let Some(hit) = self.snapshots.get_fresh(&q.namespace, now) {
            return hit;
        }
        let mut namespaces = self.namespaces.lock().unwrap_or_else(|p| p.into_inner());
        let (encoded, valid_until) = match namespaces.get_mut(&q.namespace) {
            Some(cache) => {
                let peers: Vec<PeerEntry> = cache
                    .snapshot()
                    .into_iter()
                    .map(|(peer, addrs)| PeerEntry {
                        peer_id: peer.as_bytes().to_vec(),
                        addrs: addrs.iter().map(AddrMsg::from_addr).collect(),
                    })
                    .collect();
                let resp = Response {
                    error: String::new(),
                    peers,
                };
                let valid = cache
                    .earliest_expiry()
                    .unwrap_or(now + snapshot::EMPTY_SNAPSHOT_RECHECK);
                (encode_response(&resp), valid)
            }
            None => (
                encode_response(&Response::ok()),
                now + snapshot::EMPTY_SNAPSHOT_RECHECK,
            ),
        };
        self.snapshots
            .record(&q.namespace, encoded.clone(), valid_until);
        encoded
    }

    /// 快照重建次数（观测缓存命中情况）。
    pub fn snapshot_rebuild_count(&self) -> u64 {
        self.snapshots.rebuild_count()
    }
}

/// 编码应答帧；encode 到 Vec 无容量上限，不可能失败。
fn encode_response(resp: &Response) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = resp.encode(&mut buf);
    buf
}

/// 每连接注册令牌桶：起始满桶、按分钟速率回填，控制单连接注册频率（M1）。
struct RegisterLimiter {
    tokens: f64,
    capacity: f64,
    refill_per_sec: f64,
    last: Instant,
}

impl RegisterLimiter {
    fn new(per_minute: u32) -> Self {
        Self {
            tokens: f64::from(per_minute),
            capacity: f64::from(per_minute),
            refill_per_sec: f64::from(per_minute) / 60.0,
            last: Instant::now(),
        }
    }

    /// 取一个令牌；不足则拒绝（不漏桶）。
    fn try_take(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_per_sec).min(self.capacity);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// 在一条连接上持续服务：Register 校验入库（含每连接限速），Query 应答（同样限速）；流关闭即返回。
/// 读端干净收尾（查询即断的会话终结）返回 Ok，不作为服务端错误上抛——
/// 否则每条查号会话结束都在服务端留下一条错误告警（T23 周期性
/// server link ended 噪音同源）；仅协议/链路级失败返回 Err。
pub async fn serve_link(
    conn: &mut RendezvousConn,
    registry: &RendezvousRegistry,
) -> Result<(), RendezvousError> {
    let mut register_limiter = RegisterLimiter::new(RATE_LIMIT_PER_MINUTE);
    let mut query_limiter = RegisterLimiter::new(RATE_LIMIT_QUERIES_PER_MINUTE);
    loop {
        let req = match conn.recv_msg::<Request>().await {
            Ok(req) => req,
            Err(RendezvousError::Closed) => return Ok(()),
            Err(e) => return Err(e),
        };
        let reply: Vec<u8> = match req.kind {
            Some(request::Kind::Register(reg)) => {
                if !register_limiter.try_take() {
                    encode_response(&Response::error("register rate limit exceeded"))
                } else {
                    encode_response(&match registry.register(&reg, unix_now()) {
                        Ok(()) => Response::ok(),
                        Err(e) => Response::error(e),
                    })
                }
            }
            Some(request::Kind::Query(q)) => {
                if !query_limiter.try_take() {
                    encode_response(&Response::error("query rate limit exceeded"))
                } else {
                    registry.query_encoded(&q)
                }
            }
            None => encode_response(&Response::error("missing request kind")),
        };
        conn.send_raw(reply).await?;
    }
}

#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod multi_query_tests;
#[cfg(test)]
mod tests;
