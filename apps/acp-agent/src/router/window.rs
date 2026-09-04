//! Router 的窗口与投递侧：续连窗口、权限瀑布路由、补放与退出阶梯收尾。

use std::io;
use std::time::Instant;

use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::process::Child;
use tokio::sync::mpsc::Sender;

use crate::audit::AuditEvent;
use crate::permission::{self, Decision, PermissionRequest};
use crate::reattach;
use crate::subprocess;

use super::{Exit, Flow, Outstanding};

impl super::Router {
    /// 窗口期 = 无输出面，或续连已接线但仍在等 initialize 触发补放。
    pub(super) fn window_active(&self) -> bool {
        self.sink.is_none() || self.replay_pending
    }

    pub(super) async fn attach(&mut self, sink: Sender<Vec<u8>>, defer: bool, conn: String) {
        self.sink = Some(sink.clone());
        self.window_deadline = None;
        self.replay_pending = defer;
        self.conn = conn;
        self.ever_attached = true;
        let premain: Vec<Vec<u8>> = self.premain.drain(..).collect();
        for line in premain {
            if sink.send(line).await.is_err() {
                tracing::warn!(peer = %self.params.peer_id, "wire sink closed during premain flush");
                self.enter_window().await;
                return;
            }
        }
    }

    /// 断流入口：无人值守 = 拒绝。outstanding 立即 reject-once，开窗口计时。
    pub(super) async fn enter_window(&mut self) {
        self.sink = None;
        self.replay_pending = false;
        let drained: Vec<Outstanding> = std::mem::take(&mut self.outstanding);
        for outstanding in drained {
            let response = permission::rejected_response(&outstanding.id);
            if self.write_stdin(response.as_bytes()).await.is_err() {
                tracing::warn!(peer = %self.params.peer_id, "reject-once write failed; child likely gone");
                continue;
            }
            self.audit_perm("unanswered-rejected", &outstanding.id.to_string());
        }
        if self.window_deadline.is_none() {
            self.window_deadline = Some(Instant::now() + self.params.config.window());
        }
    }

    /// 补放：桥约定宣告行 + 环形缓存内容，之后新 update 直接透传。
    pub(super) async fn replay_now(&mut self) {
        self.replay_pending = false;
        let Some(sink) = self.sink.clone() else {
            return;
        };
        let drained = self.cache.drain();
        let count = drained.len();
        let mut batch = Vec::with_capacity(count + 1);
        batch.push(reattach::replay_announcement(count));
        batch.extend(drained);
        for line in batch {
            if sink.send(line).await.is_err() {
                tracing::warn!(peer = %self.params.peer_id, "wire sink closed during replay");
                self.enter_window().await;
                return;
            }
        }
        self.params.audit.record(AuditEvent::ReattachAccepted {
            peer: self.params.peer_id.clone(),
            conn: self.conn.clone(),
            detail: format!("replayed={count}"),
        });
    }

    pub(super) async fn on_permission(&mut self, req: PermissionRequest, bytes: &[u8]) -> Flow {
        match permission::decide(&req, self.params.grant.ask_route) {
            Decision::AutoAllow(response) => {
                self.answer_locally(&response, "auto-allowed", &req.id)
                    .await
            }
            Decision::OwnerLocal(response) => {
                self.answer_locally(&response, "owner-local", &req.id).await
            }
            Decision::Forward => self.forward_permission(req, bytes).await,
        }
    }

    pub(super) async fn answer_locally(
        &mut self,
        response: &str,
        action: &str,
        id: &Value,
    ) -> Flow {
        if self.write_stdin(response.as_bytes()).await.is_err() {
            return Flow::Stop(Exit::ChildGone);
        }
        self.audit_perm(action, &id.to_string());
        Flow::Continue
    }

    pub(super) async fn forward_permission(
        &mut self,
        req: PermissionRequest,
        bytes: &[u8],
    ) -> Flow {
        if self.sink.is_none() {
            let response = permission::rejected_response(&req.id);
            return self
                .answer_locally(&response, "unanswered-rejected", &req.id)
                .await;
        }
        self.outstanding.push(Outstanding {
            deadline: Instant::now() + self.params.config.permission_timeout(),
            id: req.id.clone(),
        });
        self.audit_perm("forwarded", &req.id.to_string());
        self.forward(bytes).await
    }

    pub(super) async fn forward(&mut self, bytes: &[u8]) -> Flow {
        if !self.ever_attached {
            self.premain.push_back(bytes.to_vec());
            return Flow::Continue;
        }
        let Some(sink) = self.sink.clone() else {
            tracing::warn!(
                peer = %self.params.peer_id,
                bytes = bytes.len(),
                "child line dropped in window (non-update, bridge convention)",
            );
            return Flow::Continue;
        };
        if sink.send(bytes.to_vec()).await.is_err() {
            self.enter_window().await;
        }
        Flow::Continue
    }

    pub(super) fn cache_update(&mut self, root: &Value, bytes: &[u8]) {
        match reattach::update_session_key(root) {
            Some(session) => {
                let dropped = self.cache.push(session, bytes.to_vec());
                if dropped > 0 {
                    tracing::warn!(
                        peer = %self.params.peer_id,
                        session,
                        dropped,
                        "update cache overflow; oldest dropped",
                    );
                }
            }
            None => {
                tracing::warn!(peer = %self.params.peer_id, "session/update without sessionId dropped in window");
            }
        }
    }

    /// stdin 是行协议：代答行可能不带行尾，这里统一补齐后再刷。
    pub(super) async fn write_stdin(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.stdin.write_all(bytes).await?;
        if bytes.last() != Some(&b'\n') {
            self.stdin.write_all(b"\n").await?;
        }
        self.stdin.flush().await
    }

    pub(super) fn settle(&mut self, id: &Value) {
        self.outstanding.retain(|o| &o.id != id);
    }

    pub(super) fn audit_perm(&self, action: &str, detail: &str) {
        self.params.audit.record(AuditEvent::PermissionActed {
            peer: self.params.peer_id.clone(),
            conn: self.conn.clone(),
            action: action.to_owned(),
            detail: detail.to_owned(),
        });
    }

    pub(super) fn next_deadline(&self) -> Option<Instant> {
        let perms = self.outstanding.iter().map(|o| o.deadline).min();
        match (perms, self.window_deadline) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// 退出阶梯：stdin EOF（干净 quiesce 机会）-> 宽限等待 -> SIGKILL；随后摘除簿记。
    pub(super) async fn finish(self, child: Child, exit: Exit) {
        if let Exit::Guardrail(reason) = &exit {
            tracing::error!(peer = %self.params.peer_id, reason, "child line guardrail breached");
        }
        drop(self.stdin);
        let detail = subprocess::reap(child, self.params.config.grace()).await;
        self.params.audit.record(AuditEvent::SubprocessExit {
            peer: self.params.peer_id.clone(),
            conn: self.conn.clone(),
            detail: detail.clone(),
        });
        self.params.book.remove(&self.params.ticket);
        tracing::info!(peer = %self.params.peer_id, ?exit, detail, "router exit; subprocess reaped");
    }
}
