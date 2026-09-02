//! rendezvous 服务端：按 namespace 维护带 TTL 的注册表，校验签名后应答查询。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use p2p_identity::PeerId;
use p2p_transport::TransportAddr;

use crate::cache::MemCache;
use crate::rendezvous::link::{RendezvousConn, RendezvousError};
use crate::rendezvous::messages::{
    request, unix_now, verify_register, AddrMsg, PeerEntry, Query, Register, Request, Response,
};
use crate::AddrCache;

/// namespace 名称长度上限（字节），防恶意超长键撑内存（M1）。
pub const MAX_NAMESPACE_LEN: usize = 64;
/// 单个 namespace 的 peer 数上限（M1）。
pub const MAX_PEERS_PER_NAMESPACE: usize = 512;
/// 单条注册 TTL 上限（秒），防 136 年长占（H1，对齐 relay MAX_TTL_SECS）。
pub const MAX_TTL_SECS: u64 = 3600;
/// 每连接注册频率上限（次/分），令牌桶（M1）。
pub const RATE_LIMIT_PER_MINUTE: u32 = 10;

/// rendezvous 注册表：namespace → 带 TTL 的地址缓存。
#[derive(Default)]
pub struct RendezvousRegistry {
    namespaces: Mutex<HashMap<String, MemCache>>,
}

impl RendezvousRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 处理注册：namespace/签名新鲜度/资源上限校验通过后入库；失败返回 Err 并留日志。
    pub fn register(&self, reg: &Register, now: u64) -> Result<(), String> {
        if reg.namespace.is_empty() {
            return Err("empty namespace".to_string());
        }
        if reg.namespace.len() > MAX_NAMESPACE_LEN {
            return Err("namespace too long".to_string());
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
        let mut map = self.namespaces.lock().unwrap_or_else(|p| p.into_inner());
        let cache = map.entry(reg.namespace.clone()).or_default();
        // 每 namespace peer 数上限：仅新增且已满才拒，覆盖已有 peer 不受限
        if cache.get(&peer).is_none() && cache.snapshot().len() >= MAX_PEERS_PER_NAMESPACE {
            return Err("namespace peer limit reached".to_string());
        }
        cache.put(peer, addrs, ttl);
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
        let peers = cache
            .snapshot()
            .into_iter()
            .filter(|(peer, _)| target.is_none_or(|t| *peer == t))
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

/// 在一条连接上持续服务：Register 校验入库（含每连接限速），Query 应答；流关闭即返回。
pub async fn serve_link(
    conn: &mut RendezvousConn,
    registry: &RendezvousRegistry,
) -> Result<(), RendezvousError> {
    let mut limiter = RegisterLimiter::new(RATE_LIMIT_PER_MINUTE);
    loop {
        let req = conn.recv_msg::<Request>().await?;
        let resp = match req.kind {
            Some(request::Kind::Register(reg)) => {
                if !limiter.try_take() {
                    Response::error("register rate limit exceeded".to_string())
                } else {
                    match registry.register(&reg, unix_now()) {
                        Ok(()) => Response::ok(),
                        Err(e) => Response::error(e),
                    }
                }
            }
            Some(request::Kind::Query(q)) => registry.query(&q),
            None => Response::error("missing request kind".to_string()),
        };
        conn.send_msg(&resp).await?;
    }
}

#[cfg(test)]
mod tests;
