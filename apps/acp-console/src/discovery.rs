//! 发现清单（需求 D）：mDNS 局域网 + rendezvous 查询 + 手动 PeerId/地址添加。
//! 三路候选聚合到一处；每次清单变更输出 stdout JSON 行（CLI 可读），
//! 快照另经 status 端点 /discovery 可查。来源标注：mdns / rendezvous / manual。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use p2p_swarm::{AddrSource, NodeEvent};
use serde::Serialize;
use tokio::sync::broadcast;

use crate::out;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub peer: String,
    pub addrs: Vec<String>,
    pub source: String,
}

/// 聚合候选表：peer → 地址与来源。锁中毒时保留数据继续（into_inner），不静默清空。
#[derive(Default)]
pub struct DiscoveryHub {
    peers: Mutex<BTreeMap<String, Candidate>>,
}

impl DiscoveryHub {
    /// 登记候选；清单确有变化时输出 stdout 事件行。
    pub fn record(&self, peer: String, addrs: Vec<String>, source: &str) {
        let addr_set: BTreeSet<String> = addrs.into_iter().collect();
        let addrs: Vec<String> = addr_set.into_iter().collect();
        let changed = {
            let mut guard = match self.peers.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            let changed = match guard.get(&peer) {
                Some(existing) => existing.addrs != addrs || existing.source != source,
                None => true,
            };
            guard.insert(
                peer.clone(),
                Candidate {
                    peer,
                    addrs,
                    source: source.to_string(),
                },
            );
            changed
        };
        // 快照在锁外取：record 持锁时再取锁（std Mutex 不可重入）会自死锁。
        if changed {
            out::event(
                "discovery",
                &DiscoveryPayload {
                    peers: self.snapshot(),
                },
            );
        }
    }

    /// 按 PeerId 排序的完整快照（stdout 行与 status 端点共用形状）。
    pub fn snapshot(&self) -> Vec<Candidate> {
        let guard = match self.peers.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.values().cloned().collect()
    }
}

#[derive(Serialize)]
struct DiscoveryPayload {
    peers: Vec<Candidate>,
}

/// 底座发现事件 → 候选表转发；通道关闭即退出，滞后显式告警。
pub async fn forward_events(node: Arc<p2p::Node>, hub: Arc<DiscoveryHub>) {
    let mut rx = node.events();
    loop {
        match rx.recv().await {
            Ok(NodeEvent::PeerDiscovered {
                peer,
                addrs,
                source,
            }) => {
                hub.record(peer.to_string(), addrs, source_name(source));
            }
            Ok(_) => {}
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                tracing::warn!(skipped, "discovery events lagged; candidates may be stale");
            }
            Err(broadcast::error::RecvError::Closed) => {
                tracing::info!("discovery event stream closed");
                return;
            }
        }
    }
}

/// 手动登记（D 的手动面）：地址入底座地址簿（直拨入口）+ 候选表；
/// 随后按 PeerId 向 rendezvous 精确查号补地址（best-effort，失败降 debug 留痕）。
pub async fn apply_manual(node: &p2p::Node, hub: &DiscoveryHub, manual: &[(String, Vec<String>)]) {
    for (peer_raw, addrs) in manual {
        let peer = match crate::dial::parse_peer_id(peer_raw) {
            Ok(p) => p,
            Err(err) => {
                tracing::warn!(peer = %peer_raw, %err, "manual peer rejected");
                continue;
            }
        };
        for addr in addrs {
            if let Err(err) = node.add_peer_address(peer, addr) {
                tracing::warn!(peer = %peer_raw, addr, error = %err, "manual address rejected");
            }
        }
        hub.record(peer_raw.clone(), addrs.clone(), "manual");
        match node.query_peer(peer_raw).await {
            Ok(found) if !found.is_empty() => hub.record(peer_raw.clone(), found, "rendezvous"),
            Ok(_) => {}
            Err(err) => {
                tracing::debug!(peer = %peer_raw, error = %err, "rendezvous query unavailable");
            }
        }
    }
}

fn source_name(source: AddrSource) -> &'static str {
    match source {
        AddrSource::Mdns => "mdns",
        AddrSource::Rendezvous => "rendezvous",
        AddrSource::Manual => "manual",
    }
}
