//! 访侧会话审计（gui-contract §19.2，记录方=访侧）：一条隧道流一条记录
//! （sessionId=票据 uid，与被访侧日志同源）；字节数为访侧视角（bytes_in=
//! 自隧道收到，bytes_out=向隧道发出）。迁移自
//! apps/gui/src-tauri/src/tunnel/audit.rs（ConnAudit/AuditLog），快照元素
//! 换型为 crate [TunnelAuditRecord]（§5.2 八字段闭集）。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::Mutex;

use super::pump::PumpAudit;
use crate::audit::{TunnelAuditOutcome, TunnelAuditRecord};
use crate::wire::TunnelErrorCode;

/// 单连接审计记录：建立即入册，收尾落终态（finish 幂等，首写生效）。
pub(crate) struct SessionAudit {
    pub uid: String,
    peer_id: String,
    target: String,
    started_at: u64,
    bytes_in: AtomicU64,
    bytes_out: AtomicU64,
    end: Mutex<Option<(u64, String)>>,
}

impl SessionAudit {
    pub(crate) fn new(uid: String, peer_id: String, target: String) -> Self {
        Self {
            uid,
            peer_id,
            target,
            started_at: unix_secs(),
            bytes_in: AtomicU64::new(0),
            bytes_out: AtomicU64::new(0),
            end: Mutex::new(None),
        }
    }

    /// 终态：ok=干净收尾；code=错误码闭集字面量。幂等（首写生效）。
    pub(crate) async fn finish(&self, outcome: &str) {
        let mut end = self.end.lock().await;
        if end.is_none() {
            *end = Some((unix_secs(), outcome.to_string()));
        }
    }

    /// 物化为 §5.2 八字段闭集记录。存续期（end 未落）：`ended_at=0` 哨兵承载
    /// §5.2「ended_at 在会话存续期为空」，outcome 为占位 Served（无意义）——
    /// 消费方以 `ended_at == 0` 判「open」；TunnelAuditOutcome 无 open 变体
    /// （p2pctl 对其穷尽匹配，加变体越禁区，登记 tb-PROGRESS 漂移）。
    pub(crate) async fn record(&self) -> TunnelAuditRecord {
        let (ended_at, outcome) = match self.end.lock().await.clone() {
            Some((at, code)) => (at, session_outcome(&code)),
            None => (0, TunnelAuditOutcome::Served),
        };
        TunnelAuditRecord {
            session_id: self.uid.clone(),
            peer_id: self.peer_id.clone(),
            target: self.target.clone(),
            started_at: self.started_at,
            ended_at,
            bytes_in: self.bytes_in.load(Ordering::Relaxed),
            bytes_out: self.bytes_out.load(Ordering::Relaxed),
            outcome,
        }
    }
}

impl PumpAudit for SessionAudit {
    fn add_in(&self, n: u64) {
        self.bytes_in.fetch_add(n, Ordering::Relaxed);
    }

    fn add_out(&self, n: u64) {
        self.bytes_out.fetch_add(n, Ordering::Relaxed);
    }
}

/// finish 口径（"ok" | TunnelErrorCode 字面量，conn.rs failure_code 产出）
/// → crate 终态。非 "ok" 一律 Broken(code)（访侧记录方语义：未干净收口即
/// 故障）；未知字面量兜底 Io——产出为闭集（from_wire 全覆盖），不可达不静默。
fn session_outcome(code: &str) -> TunnelAuditOutcome {
    match code {
        "ok" => TunnelAuditOutcome::Served,
        other => match TunnelErrorCode::from_wire(other) {
            Some(code) => TunnelAuditOutcome::Broken(code),
            None => TunnelAuditOutcome::Broken(TunnelErrorCode::Io),
        },
    }
}

/// 反代生命周期内的全部会话记录（进程内，不落盘——持久审计在被访侧）。
#[derive(Default)]
pub(crate) struct SessionLog {
    records: Mutex<Vec<Arc<SessionAudit>>>,
}

impl SessionLog {
    pub(crate) async fn push(&self, record: Arc<SessionAudit>) {
        self.records.lock().await.push(record);
    }

    /// 快照（ProxyCtx::audit_snapshot 的取数面）。
    pub(crate) async fn snapshot(&self) -> Vec<TunnelAuditRecord> {
        let records = self.records.lock().await;
        let mut out = Vec::with_capacity(records.len());
        for record in records.iter() {
            out.push(record.record().await);
        }
        out
    }
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn audit_lifecycle_open_then_ok() {
        let log = SessionLog::default();
        let record = Arc::new(SessionAudit::new(
            "0123456789abcdef".into(),
            "peer".into(),
            "127.0.0.1:3080".into(),
        ));
        log.push(Arc::clone(&record)).await;
        record.add_in(11);
        record.add_out(7);
        let snap = log.snapshot().await;
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].ended_at, 0, "存续期 ended_at 为空哨兵 0");
        assert_eq!(snap[0].bytes_in, 11);
        assert_eq!(snap[0].bytes_out, 7);
        record.finish("ok").await;
        let snap = log.snapshot().await;
        assert!(matches!(snap[0].outcome, TunnelAuditOutcome::Served));
        assert_ne!(snap[0].ended_at, 0);
    }

    #[tokio::test]
    async fn finish_is_first_write_wins() {
        let record = SessionAudit::new("f".repeat(16), "p".into(), "t".into());
        record.finish("io").await;
        record.finish("ok").await;
        let record = record.record().await;
        assert!(matches!(
            record.outcome,
            TunnelAuditOutcome::Broken(TunnelErrorCode::Io)
        ));
        assert_ne!(record.ended_at, 0);
    }
}
