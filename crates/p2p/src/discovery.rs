//! 发现事件接线：DiscoveryEvent → 地址簿 + NodeEvent（design §7/§12）。

use std::sync::Arc;

use p2p_discovery::{DiscoveryEvent, Source};
use p2p_swarm::{AddrSource, Swarm};
use tokio::sync::mpsc;

/// 失败留痕级别判定（E4 刷屏治理）：同一故障序列仅首次 WARN，
/// 重复失败降级 debug；失败信号不丢（首次告警 + debug 可追踪）。
pub(crate) fn failure_notice_level(already_noticed: bool) -> tracing::Level {
    if already_noticed {
        tracing::Level::DEBUG
    } else {
        tracing::Level::WARN
    }
}

/// 各发现源的失败告警状态：同源连续失败仅首次 WARN，重复失败降级 debug；
/// 该源重新发现成功即复位，下次故障仍能触发告警（rendezvous 盲拨留痕同用此策略）。
#[derive(Default)]
struct SourceFailureNotices([bool; 3]);

impl SourceFailureNotices {
    /// 源重新发现成功：复位该源故障序列。
    fn on_discovered(&mut self, source: Source) {
        self.0[source_slot(source)] = false;
    }

    /// 源失败：按首次/重复分级留痕，返回实际落盘级别（供断言）。
    fn on_failed(&mut self, source: Source, reason: &str) -> tracing::Level {
        let slot = source_slot(source);
        let level = failure_notice_level(self.0[slot]);
        self.0[slot] = true;
        if level == tracing::Level::WARN {
            tracing::warn!(source = ?source, %reason, "discovery source failed");
        } else {
            tracing::debug!(source = ?source, %reason, "discovery source failed");
        }
        level
    }
}

/// Source → 状态槽位。
fn source_slot(source: Source) -> usize {
    match source {
        Source::Mdns => 0,
        Source::Rendezvous => 1,
        Source::Cache => 2,
    }
}

/// 转发发现事件：新地址入地址簿（触发 PeerDiscovered），
/// 过期发 PeerDisconnected（design §7.1），源失败留告警不打断其他源。
pub(crate) async fn forward_discovery(mut rx: mpsc::Receiver<DiscoveryEvent>, swarm: Arc<Swarm>) {
    let mut notices = SourceFailureNotices::default();
    while let Some(ev) = rx.recv().await {
        match ev {
            DiscoveryEvent::Discovered(dp) => {
                let source = match dp.source {
                    Source::Mdns => AddrSource::Mdns,
                    Source::Rendezvous | Source::Cache => AddrSource::Rendezvous,
                };
                notices.on_discovered(dp.source);
                swarm.add_peer_addresses_with_source(dp.peer, dp.addrs, source);
            }
            DiscoveryEvent::Expired(peer) => swarm.on_peer_expired(peer),
            DiscoveryEvent::Failed { source, reason } => {
                notices.on_failed(source, &reason);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_notice_level_first_only() {
        // E4 刷屏治理（检查轮12）：同一故障序列首次 WARN，重复降级 debug
        assert_eq!(failure_notice_level(false), tracing::Level::WARN);
        assert_eq!(failure_notice_level(true), tracing::Level::DEBUG);
    }

    #[test]
    fn source_notices_warn_once_per_outage_then_reset_on_rediscovery() {
        let mut n = SourceFailureNotices::default();
        assert_eq!(
            n.on_failed(Source::Rendezvous, "dial refused"),
            tracing::Level::WARN
        );
        assert_eq!(
            n.on_failed(Source::Rendezvous, "dial refused"),
            tracing::Level::DEBUG,
            "同源连续失败重复触发必须降级 debug"
        );
        // 其他源互不影响
        assert_eq!(
            n.on_failed(Source::Mdns, "daemon down"),
            tracing::Level::WARN
        );
        // 重新发现成功即复位：下次故障重新告警
        n.on_discovered(Source::Rendezvous);
        assert_eq!(
            n.on_failed(Source::Rendezvous, "dial refused"),
            tracing::Level::WARN,
            "复位后必须重新 WARN"
        );
    }
}
