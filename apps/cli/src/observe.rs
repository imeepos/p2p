//! 守护进程侧观测注册表（F10）：订阅 NodeEvent 聚合地址簿与在线态，
//! 归约语义对齐 GUI event-reducer：lastSeen 只认正向证据（发现源消息与
//! 连接成功），manual 来源的 peer_discovered 是本端自身登记，不构成对端
//! 存活证据。只读命令（peer list / discovery list）经控制通道读本表。

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use p2p::{Node, NodeEvent};
use p2p_swarm::AddrSource;
use serde::{Deserialize, Serialize};

use crate::daemon::now_ms;

/// 地址簿单条目（peer list / discovery list 共用事实源）。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PeerEntry {
    pub peer_id: String,
    pub addrs: Vec<String>,
    /// 聚合来源：mdns | rendezvous | manual（发现痕迹优先，同 GUI）。
    pub source: String,
    pub connected: bool,
    pub last_seen_ms: u64,
    /// 首次进入地址簿时刻（发现面「最早发现」口径，同 GUI discovered 表）。
    pub first_seen_ms: u64,
}

/// 来源计数（discovery list 汇总行）。
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryStats {
    pub total: usize,
    pub connected: usize,
    pub mdns: usize,
    pub rendezvous: usize,
    pub manual: usize,
}

/// 事件归约的观测注册表；单锁覆盖读写，量级为对端数，无争用风险。
#[derive(Default)]
pub struct PeerRegistry {
    peers: Mutex<BTreeMap<String, PeerEntry>>,
}

impl PeerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 归约一条节点事件（语义见模块注释）。
    pub fn observe(&self, event: &NodeEvent) {
        let mut peers = self.peers.lock().expect("registry lock");
        match event {
            NodeEvent::PeerDiscovered { peer, addrs, source } => {
                // manual 是本端自身登记，不刷新 lastSeen（非对端存活证据）。
                let fresh = *source != AddrSource::Manual;
                let entry = peers.entry(peer.to_string()).or_insert_with(|| PeerEntry {
                    peer_id: peer.to_string(),
                    addrs: Vec::new(),
                    source: source_name(source).into(),
                    connected: false,
                    last_seen_ms: 0,
                    first_seen_ms: now_ms(),
                });
                entry.addrs = addrs.clone();
                entry.source = source_name(source).into();
                if fresh {
                    entry.last_seen_ms = now_ms();
                }
            }
            NodeEvent::PeerConnected { peer } => {
                let entry = entry_or_new(&mut peers, peer);
                entry.connected = true;
                entry.last_seen_ms = now_ms();
            }
            NodeEvent::PeerDisconnected { peer } => {
                // 断开可能来自发现缓存 TTL 过期，不是负向活性证据：只翻 connected。
                if let Some(entry) = peers.get_mut(&peer.to_string()) {
                    entry.connected = false;
                }
            }
            _ => {}
        }
    }

    /// 地址簿快照：peerId 字典序，输出稳定可 grep。
    pub fn snapshot(&self) -> Vec<PeerEntry> {
        self.peers.lock().expect("registry lock").values().cloned().collect()
    }

    /// 来源计数（汇总行事实源）。
    pub fn stats(&self) -> RegistryStats {
        let peers = self.peers.lock().expect("registry lock");
        let mut stats = RegistryStats {
            total: peers.len(),
            ..RegistryStats::default()
        };
        for entry in peers.values() {
            if entry.connected {
                stats.connected += 1;
            }
            match entry.source.as_str() {
                "mdns" => stats.mdns += 1,
                "rendezvous" => stats.rendezvous += 1,
                _ => stats.manual += 1,
            }
        }
        stats
    }
}

fn entry_or_new<'a>(
    peers: &'a mut BTreeMap<String, PeerEntry>,
    peer: &p2p::PeerId,
) -> &'a mut PeerEntry {
    peers.entry(peer.to_string()).or_insert_with(|| PeerEntry {
        peer_id: peer.to_string(),
        addrs: Vec::new(),
        source: "manual".into(),
        connected: false,
        last_seen_ms: 0,
        first_seen_ms: now_ms(),
    })
}

fn source_name(source: &AddrSource) -> &'static str {
    match source {
        AddrSource::Mdns => "mdns",
        AddrSource::Rendezvous => "rendezvous",
        AddrSource::Manual => "manual",
    }
}

/// 启动事件采集任务：归约直到事件通道关闭（守护进程退出）。
pub fn spawn_collector(node: &Node, registry: Arc<PeerRegistry>) {
    let mut rx = node.events();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => registry.observe(&event),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("p2pctl-daemon: 观测注册表落后 {n} 条事件，对端痕迹可能缺失");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p::PeerId;

    fn peer(n: u8) -> PeerId {
        PeerId::from_bytes([n; 32])
    }

    #[test]
    fn manual_discovery_does_not_prove_liveness() {
        let reg = PeerRegistry::new();
        reg.observe(&NodeEvent::PeerDiscovered {
            peer: peer(1),
            addrs: vec!["127.0.0.1/u3400".into()],
            source: AddrSource::Manual,
        });
        let entry = &reg.snapshot()[0];
        assert_eq!(entry.source, "manual");
        assert!(!entry.connected);
        assert_eq!(entry.last_seen_ms, 0, "manual 来源不算对端存活证据");
    }

    #[test]
    fn mdns_discovery_refreshes_last_seen() {
        let reg = PeerRegistry::new();
        reg.observe(&NodeEvent::PeerDiscovered {
            peer: peer(2),
            addrs: vec!["192.168.1.5/u3400".into()],
            source: AddrSource::Mdns,
        });
        assert!(reg.snapshot()[0].last_seen_ms > 0);
        assert_eq!(reg.stats().mdns, 1);
    }

    #[test]
    fn connected_flip_and_disconnect_keeps_book() {
        let reg = PeerRegistry::new();
        reg.observe(&NodeEvent::PeerDiscovered {
            peer: peer(3),
            addrs: vec!["10.0.0.1/t3400".into()],
            source: AddrSource::Manual,
        });
        reg.observe(&NodeEvent::PeerConnected { peer: peer(3) });
        assert!(reg.snapshot()[0].connected);
        assert_eq!(reg.stats().connected, 1);
        reg.observe(&NodeEvent::PeerDisconnected { peer: peer(3) });
        let entry = &reg.snapshot()[0];
        assert!(!entry.connected, "断开只翻在线位");
        assert!(!entry.addrs.is_empty(), "地址簿条目保留");
    }

    #[test]
    fn stats_counts_by_source() {
        let reg = PeerRegistry::new();
        for (n, source) in [
            (4u8, AddrSource::Mdns),
            (5, AddrSource::Rendezvous),
            (6, AddrSource::Manual),
        ] {
            reg.observe(&NodeEvent::PeerDiscovered {
                peer: peer(n),
                addrs: vec!["10.0.0.2/u1".into()],
                source,
            });
        }
        let stats = reg.stats();
        assert_eq!((stats.total, stats.mdns, stats.rendezvous, stats.manual), (3, 1, 1, 1));
    }
}