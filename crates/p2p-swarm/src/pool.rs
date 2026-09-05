//! 连接池：PeerId → 已升级连接，幂等 connect 与并发拨号合并（coordination.md S 包）。
//! E8：池条目携带使用记账与连接类别；空闲回收见 reclaim_idle（调研建议 4）。

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use p2p_identity::PeerId;
use p2p_mux::MuxControl;
use tokio::sync::Mutex as AsyncMutex;

use crate::usage::{unix_now, ConnUsage};

type Mux = Arc<dyn MuxControl>;

/// 连接类别（E8）：中继电路断链须喂活跃度判定（RelaySlot 源），
/// 直连断链由状态机全权处理（见 liveness.rs 模块注释）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnKind {
    Direct,
    RelayCircuit,
}

/// 连接方向（收敛策略的输入）：与 dial.rs 的 ConnDirection 同义，收窄在
/// 本池内定义以免反向依赖 swarm 模块（pub(super) 不可见）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PoolDirection {
    Inbound,
    Outbound,
}

/// 池条目：连接本体 + 使用记账（空闲回收的「使用中」判据）+ 方向；类别在
/// 入池时由调用方直接交给 serve 循环，不落条目。
pub(crate) struct PoolEntry {
    pub(crate) mux: Mux,
    pub(crate) usage: Arc<ConnUsage>,
    pub(crate) direction: PoolDirection,
}

/// admit 的裁决结果：Accepted 空池直收；Replaced 新连接顶替旧连接；
/// RejectedExisting 新连接落选（原样带回，调用方 close）。
/// Mux 非 Debug，手工实现只报变体与是否带连接，不展开内容。
pub enum Admission {
    Accepted,
    /// 被顶替连接随裁决值丢弃：Arc 清零即断 yamux 驱动，生产无需显式 close；
    /// 载荷仅供测试断言顶替关系。
    Replaced(#[cfg_attr(not(test), allow(dead_code))] Mux),
    RejectedExisting(Mux),
}

impl std::fmt::Debug for Admission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Admission::Accepted => f.write_str("Accepted"),
            Admission::Replaced(_) => f.write_str("Replaced(..)"),
            Admission::RejectedExisting(_) => f.write_str("RejectedExisting(..)"),
        }
    }
}

/// 单槽连接池：每 peer 至多一条在册连接；重复入池让位于先到者。
pub struct ConnectionPool {
    conns: Mutex<HashMap<PeerId, PoolEntry>>,
    dial_locks: Mutex<HashMap<PeerId, Arc<AsyncMutex<()>>>>,
}

impl ConnectionPool {
    pub fn new() -> Self {
        Self {
            conns: Mutex::new(HashMap::new()),
            dial_locks: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, peer: &PeerId) -> Option<Mux> {
        self.conns
            .lock()
            .expect("pool lock")
            .get(peer)
            .map(|e| e.mux.clone())
    }

    /// 在册连接的使用记账（业务流触点：touch 与在途守护）。
    pub(crate) fn usage(&self, peer: &PeerId) -> Option<Arc<ConnUsage>> {
        self.conns
            .lock()
            .expect("pool lock")
            .get(peer)
            .map(|e| e.usage.clone())
    }

    /// 在册连接数（指标水位用）。
    pub fn len(&self) -> usize {
        self.conns.lock().expect("pool lock").len()
    }

    /// 是否无在册连接（测试断言用；生产走 metrics 水位）。
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 幂等插入：peer 已在册时丢弃新连接并返回 false（先到者优先）。
    /// 生产路径走 [Self::admit_as] 收敛裁决；本方法供测试铺底。
    /// 方向固定记 Outbound：测试铺底不关心方向语义。
    #[cfg(test)]
    pub fn insert(&self, peer: PeerId, mux: Mux) -> bool {
        let mut conns = self.conns.lock().expect("pool lock");
        if conns.contains_key(&peer) {
            return false;
        }
        conns.insert(peer, self.entry(mux, PoolDirection::Outbound));
        true
    }

    fn entry(&self, mux: Mux, direction: PoolDirection) -> PoolEntry {
        PoolEntry {
            mux,
            usage: Arc::new(ConnUsage::new(unix_now())),
            direction,
        }
    }

    /// 在册连接的方向（收敛策略判断同向重拨用）；不在册返回 None。
    pub(crate) fn entry_direction(&self, peer: &PeerId) -> Option<PoolDirection> {
        self.conns
            .lock()
            .expect("pool lock")
            .get(peer)
            .map(|e| e.direction)
    }

    /// 收敛裁决原子入口（测试用）：空池直收；冲突时按 prefer_new 二选一。
    /// 生产走 [Self::admit_as] 取使用记账挂业务流。
    #[cfg(test)]
    pub fn admit(&self, peer: PeerId, mux: Mux, prefer_new: bool) -> Admission {
        self.admit_as(peer, mux, PoolDirection::Outbound, prefer_new)
            .0
    }

    /// 收敛裁决并返回胜者条目的使用记账（落选为 None），
    /// 供 serve 循环把业务流挂在正确连接的计数上（E8）。
    /// 方向随条目落池，供下次收敛判断「同向重拨」。
    pub(crate) fn admit_as(
        &self,
        peer: PeerId,
        mux: Mux,
        direction: PoolDirection,
        prefer_new: bool,
    ) -> (Admission, Option<Arc<ConnUsage>>) {
        let mut conns = self.conns.lock().expect("pool lock");
        match conns.remove(&peer) {
            None => {
                let entry = self.entry(mux, direction);
                let usage = entry.usage.clone();
                conns.insert(peer, entry);
                (Admission::Accepted, Some(usage))
            }
            Some(old) => {
                if prefer_new {
                    let entry = self.entry(mux, direction);
                    let usage = entry.usage.clone();
                    conns.insert(peer, entry);
                    (Admission::Replaced(old.mux), Some(usage))
                } else {
                    let usage = old.usage.clone();
                    conns.insert(peer, old);
                    (Admission::RejectedExisting(mux), Some(usage))
                }
            }
        }
    }

    /// 仅当在册连接与 mux 同源时移除，防止误删重连后的新连接。
    pub fn remove_if_same(&self, peer: &PeerId, mux: &Mux) -> bool {
        let mut conns = self.conns.lock().expect("pool lock");
        match conns.get(peer) {
            Some(current) if Arc::ptr_eq(&current.mux, mux) => {
                conns.remove(peer);
                true
            }
            _ => false,
        }
    }

    /// 出池并返回在册连接（挂断用）；不在册返回 None。
    pub fn take(&self, peer: &PeerId) -> Option<Mux> {
        self.conns
            .lock()
            .expect("pool lock")
            .remove(peer)
            .map(|e| e.mux)
    }

    /// 清空（关停用），返回被移除的 peer 列表。
    pub fn clear(&self) -> Vec<PeerId> {
        let mut conns = self.conns.lock().expect("pool lock");
        let peers = conns.keys().copied().collect();
        conns.clear();
        peers
    }

    /// 空闲回收扫描（E8）：取走空闲超过 threshold 且无在途业务流的连接。
    /// 使用中豁免见 ConnUsage::is_idle；返回 (peer, 连接) 交调用方关闭与归因。
    pub fn reclaim_idle(&self, threshold: Duration, now_unix: u64) -> Vec<(PeerId, Mux)> {
        let mut conns = self.conns.lock().expect("pool lock");
        let mut reclaimed = Vec::new();
        conns.retain(|peer, entry| {
            if entry.usage.is_idle(threshold, now_unix) {
                reclaimed.push((*peer, entry.mux.clone()));
                false
            } else {
                true
            }
        });
        reclaimed
    }

    /// 幂等获取或拨号：已有连接直接复用；同 peer 并发请求经拨号锁合并为一次拨号。
    pub async fn get_or_dial(
        &self,
        peer: PeerId,
        dial: impl Future<Output = io::Result<Mux>>,
    ) -> io::Result<Mux> {
        if let Some(mux) = self.get(&peer) {
            return Ok(mux);
        }
        let lock = self.dial_lock(&peer);
        let _guard = lock.lock().await;
        // 拿到拨号锁后复查：等锁期间他人可能已完成拨号
        if let Some(mux) = self.get(&peer) {
            return Ok(mux);
        }
        dial.await
    }

    fn dial_lock(&self, peer: &PeerId) -> Arc<AsyncMutex<()>> {
        self.dial_locks
            .lock()
            .expect("pool lock")
            .entry(*peer)
            .or_default()
            .clone()
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}
