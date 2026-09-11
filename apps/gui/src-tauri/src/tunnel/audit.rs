//! 访侧会话审计（gui-contract §19.2 TunnelSessionAudit，记录方=访侧）：
//! 一条隧道流一条记录（sessionId=票据 uid，与被访侧日志同源）；字节数为
//! 访侧视角（bytesIn=自隧道收到，bytesOut=向隧道发出）；outcome 取值
//! `"open" | "ok" | 六值错误码`，GUI 不得自造第四类。

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::Mutex;

use super::types::TunnelSessionAudit;

/// 单连接审计记录：建立即入册，收尾落终态。
pub struct ConnAudit {
    pub uid: String,
    pub peer_id: String,
    pub target: String,
    pub started_at: u64,
    bytes_in: AtomicU64,
    bytes_out: AtomicU64,
    end: Mutex<Option<(u64, String)>>,
}

impl ConnAudit {
    pub fn new(uid: String, peer_id: String, target: String) -> Self {
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

    pub fn add_in(&self, bytes: u64) {
        self.bytes_in.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn add_out(&self, bytes: u64) {
        self.bytes_out.fetch_add(bytes, Ordering::Relaxed);
    }

    /// 终态：ok=干净收尾；code=错误码闭集字面量。幂等（首写生效）。
    pub async fn finish(&self, outcome: &str) {
        let mut end = self.end.lock().await;
        if end.is_none() {
            *end = Some((unix_secs(), outcome.to_string()));
        }
    }

    async fn snapshot(&self) -> TunnelSessionAudit {
        let (ended_at, outcome) = match self.end.lock().await.clone() {
            Some((at, code)) => (Some(at), code),
            None => (None, "open".to_string()),
        };
        TunnelSessionAudit {
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

/// 反代生命周期内的全部会话记录（进程内，不落盘——持久审计在被访侧）。
#[derive(Default)]
pub struct AuditLog {
    records: Mutex<Vec<Arc<ConnAudit>>>,
}

impl AuditLog {
    pub async fn push(&self, record: Arc<ConnAudit>) {
        self.records.lock().await.push(record);
    }

    /// 快照（tunnel_status 的 sessions 字段）。
    pub async fn snapshot(&self) -> Vec<TunnelSessionAudit> {
        let records = self.records.lock().await;
        let mut out = Vec::with_capacity(records.len());
        for record in records.iter() {
            out.push(record.snapshot().await);
        }
        out
    }
}

/// 反代入口链接拼装（§6：open_url = http://<local_addr>/?token=<同 token>）。
pub fn open_url(local_addr: SocketAddr, token: &str) -> String {
    format!("http://{local_addr}/?token={token}")
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
        let log = AuditLog::default();
        let record = Arc::new(ConnAudit::new(
            "0123456789abcdef".into(),
            "peer".into(),
            "127.0.0.1:3080".into(),
        ));
        log.push(Arc::clone(&record)).await;
        record.add_in(11);
        record.add_out(7);
        let snap = log.snapshot().await;
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].outcome, "open");
        assert_eq!(snap[0].ended_at, None);
        assert_eq!(snap[0].bytes_in, 11);
        assert_eq!(snap[0].bytes_out, 7);
        record.finish("ok").await;
        let snap = log.snapshot().await;
        assert_eq!(snap[0].outcome, "ok");
        assert!(snap[0].ended_at.is_some());
    }

    #[tokio::test]
    async fn finish_is_first_write_wins() {
        let record = ConnAudit::new("f".repeat(16).to_string(), "p".into(), "t".into());
        record.finish("io").await;
        record.finish("ok").await;
        assert_eq!(record.snapshot().await.outcome, "io");
    }
}
