//! outbox 观测与手动补投门面（F3）：按对端列 pending/failed 与已投计数，
//! flush 手动补投并逐对端回报结果。serve 启动/周期自动补投泵在 outbox.rs，
//! 两者共用 flush_peer（同 peer 串行锁，条目不重复投递）。

use serde::Serialize;

use crate::model::{parse_peer_id, ChatError, ChatKind, ChatStatus, Sender};
use crate::outbox;
use crate::Chat;

/// 单条积压条目摘要（outbox list 展示用）。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxEntryReport {
    pub id: String,
    pub kind: ChatKind,
    pub ts_ms: i64,
    pub status: ChatStatus,
}

/// 按对端聚合的行箱视图：entries 为积压（pending/failed），delivered 为本端
/// 发出且已确认的历史条数（收到的消息不计数）。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxPeerReport {
    pub peer_id: String,
    pub pending: usize,
    pub failed: usize,
    pub delivered: usize,
    pub entries: Vec<OutboxEntryReport>,
}

/// flush 单对端结果：flushed = 本次出队条数，remaining = 未出队（对端不可达
/// 保持 pending 属正常闭环，不算命令失败），error = flush 本身异常时留观测。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxFlushPeerReport {
    pub peer_id: String,
    pub flushed: usize,
    pub remaining: usize,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxFlushReport {
    pub peers: Vec<OutboxFlushPeerReport>,
}

impl Chat {
    /// 行箱视图：好友簿全量 ∪ 含积压对端；全空对端不列。
    pub fn outbox_list(&self) -> Result<Vec<OutboxPeerReport>, ChatError> {
        let mut peers: Vec<String> = self
            .core
            .store
            .friends_list()?
            .into_iter()
            .map(|f| f.peer_id)
            .collect();
        for p in self.core.store.outbox_peers() {
            if !peers.contains(&p) {
                peers.push(p);
            }
        }
        let mut rows = Vec::new();
        for peer in peers {
            let entries: Vec<OutboxEntryReport> = self
                .core
                .store
                .outbox_for(&peer)
                .into_iter()
                .map(|e| OutboxEntryReport {
                    id: e.id,
                    kind: e.kind,
                    ts_ms: e.ts_ms,
                    status: e.status,
                })
                .collect();
            let delivered = self
                .core
                .store
                .messages_for(&peer)?
                .iter()
                .filter(|m| m.sender == Sender::Me && m.status == ChatStatus::Delivered)
                .count();
            if entries.is_empty() && delivered == 0 {
                continue;
            }
            let pending = entries
                .iter()
                .filter(|e| e.status == ChatStatus::Pending)
                .count();
            let failed = entries.len() - pending;
            rows.push(OutboxPeerReport {
                peer_id: peer,
                pending,
                failed,
                delivered,
                entries,
            });
        }
        Ok(rows)
    }

    /// 手动补投：缺省清扫全部含积压对端，--peer 指定单个（连接失败保持
    /// pending 计入 remaining，不作为命令失败）。
    pub async fn outbox_flush(&self, peer: Option<&str>) -> Result<OutboxFlushReport, ChatError> {
        let peers: Vec<String> = match peer {
            Some(p) => {
                parse_peer_id(p)?;
                vec![p.to_string()]
            }
            None => self.core.store.outbox_peers(),
        };
        let mut rows = Vec::new();
        for peer in &peers {
            let before = self.core.store.outbox_for(peer).len();
            let mut error = None;
            if before > 0 {
                if let Err(e) = outbox::flush_peer(&self.core, peer).await {
                    tracing::warn!(peer = %peer, error = %e, "outbox 手动补投失败");
                    error = Some(e.to_string());
                }
            }
            let remaining = self.core.store.outbox_for(peer).len();
            rows.push(OutboxFlushPeerReport {
                peer_id: peer.clone(),
                flushed: before.saturating_sub(remaining),
                remaining,
                error,
            });
        }
        Ok(OutboxFlushReport { peers: rows })
    }
}
