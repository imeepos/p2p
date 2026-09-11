//! 会话审计（契约 §5）：session_id / peer_id / target / started_at / ended_at /
//! bytes_in / bytes_out / outcome。成功与被拒会话均留痕（拒绝含错误码）。

use std::fmt;
use std::sync::{Mutex, MutexGuard};

use crate::wire::TunnelErrorCode;

/// 会话终态：Served = 双向自然收口；Rejected = 准入拒绝（含错误码）；
/// Broken = 建流后 IO 故障（关闭原因码，契约 §4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelAuditOutcome {
    Served,
    Rejected(TunnelErrorCode),
    Broken(TunnelErrorCode),
}

impl fmt::Display for TunnelAuditOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Served => f.write_str("served"),
            Self::Rejected(code) => write!(f, "rejected:{code}"),
            Self::Broken(code) => write!(f, "broken:{code}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelAuditRecord {
    pub session_id: String,
    pub peer_id: String,
    pub target: String,
    pub started_at: u64,
    pub ended_at: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub outcome: TunnelAuditOutcome,
}

/// 进程内审计账（会话态；持久化属产品装配层）。
#[derive(Default)]
pub struct TunnelAudit {
    records: Mutex<Vec<TunnelAuditRecord>>,
}

impl TunnelAudit {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, record: TunnelAuditRecord) {
        tracing::info!(
            session_id = %record.session_id,
            peer_id = %record.peer_id,
            target = %record.target,
            started_at = record.started_at,
            ended_at = record.ended_at,
            bytes_in = record.bytes_in,
            bytes_out = record.bytes_out,
            outcome = %record.outcome,
            "tunnel session audit"
        );
        self.lock().push(record);
    }

    pub fn snapshot(&self) -> Vec<TunnelAuditRecord> {
        self.lock().clone()
    }

    fn lock(&self) -> MutexGuard<'_, Vec<TunnelAuditRecord>> {
        // 毒化（持有方 panic）时沿用既有记录继续追加，不放大故障。
        self.records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
