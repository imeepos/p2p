//! 审计事件（设计 §7/§12-Q5）：连接建立/拒绝/门禁拒绝/子进程退出全部留痕。
//! 生产走 tracing 结构化字段（event=kind, peer, code, ts）；测试用 CaptureAudit 捕获断言。
//! 审计只记 PeerId/错误码/时间，不含策略细节与任何凭据负载。

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditEvent {
    /// 握手层拒绝（策略表未授权 / 握手非法 / peer 不可归属）。
    ConnDenied { peer: String, code: String },
    /// 资源门禁拒绝：limit 区分 total / per-peer 上限。
    GateDenied {
        peer: String,
        code: String,
        limit: &'static str,
    },
    /// 连接建立：握手 ready 已回，子进程已 spawn。
    ConnEstablished { peer: String, conn: String },
    /// 子进程 spawn 失败（断流 + 审计，不 panic）。
    SpawnFailed {
        peer: String,
        conn: String,
        detail: String,
    },
    /// 客户端断流（退出阶梯入口）。
    ClientGone { peer: String, conn: String },
    /// 子进程退出：detail 含退出状态或 killed-after-grace。
    SubprocessExit {
        peer: String,
        conn: String,
        detail: String,
    },
}

impl AuditEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ConnDenied { .. } => "conn-denied",
            Self::GateDenied { .. } => "gate-denied",
            Self::ConnEstablished { .. } => "conn-established",
            Self::SpawnFailed { .. } => "spawn-failed",
            Self::ClientGone { .. } => "client-gone",
            Self::SubprocessExit { .. } => "subprocess-exit",
        }
    }

    pub fn peer(&self) -> &str {
        match self {
            Self::ConnDenied { peer, .. }
            | Self::GateDenied { peer, .. }
            | Self::ConnEstablished { peer, .. }
            | Self::SpawnFailed { peer, .. }
            | Self::ClientGone { peer, .. }
            | Self::SubprocessExit { peer, .. } => peer,
        }
    }

    pub fn code(&self) -> &str {
        match self {
            Self::ConnDenied { code, .. } | Self::GateDenied { code, .. } => code,
            _ => "-",
        }
    }
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 审计出口。实现必须无阻塞、无 panic；落盘语义由 tracing 层承担。
pub trait AuditSink: Send + Sync {
    fn record(&self, event: AuditEvent);
}

/// 生产实现：tracing 结构化字段（p2p-log 初始化后落文件/终端）。
pub struct TracingAudit;

impl AuditSink for TracingAudit {
    fn record(&self, event: AuditEvent) {
        let ts = unix_ms();
        match &event {
            AuditEvent::ConnDenied { peer, code } => {
                tracing::warn!(target: "acp_audit", ts, event = event.kind(), peer, code, "connection denied");
            }
            AuditEvent::GateDenied { peer, code, limit } => {
                tracing::warn!(target: "acp_audit", ts, event = event.kind(), peer, code, limit, "gate denied");
            }
            AuditEvent::ConnEstablished { peer, conn } => {
                tracing::info!(target: "acp_audit", ts, event = event.kind(), peer, conn, "connection established");
            }
            AuditEvent::SpawnFailed { peer, conn, detail } => {
                tracing::error!(target: "acp_audit", ts, event = event.kind(), peer, conn, detail, "spawn failed");
            }
            AuditEvent::ClientGone { peer, conn } => {
                tracing::info!(target: "acp_audit", ts, event = event.kind(), peer, conn, "client stream closed");
            }
            AuditEvent::SubprocessExit { peer, conn, detail } => {
                tracing::info!(target: "acp_audit", ts, event = event.kind(), peer, conn, detail, "subprocess exited");
            }
        }
    }
}

/// 测试实现：内存捕获，供断言。毒锁恢复沿用 p2p-log 的 into_inner 手法。
#[derive(Default)]
pub struct CaptureAudit {
    events: Mutex<Vec<AuditEvent>>,
}

impl CaptureAudit {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> Vec<AuditEvent> {
        self.lock().clone()
    }

    pub fn contains(&self, pred: impl Fn(&AuditEvent) -> bool) -> bool {
        self.lock().iter().any(pred)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<AuditEvent>> {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl AuditSink for CaptureAudit {
    fn record(&self, event: AuditEvent) {
        self.lock().push(event);
    }
}