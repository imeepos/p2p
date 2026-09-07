//! 兑换与撤销级联（设计 §4/§3）：哈希匹配 + 状态判定 + 激活绑定，
//! 台账先落盘、策略表写失败回滚的两阶段序。

use acp_common::policy::PeerPolicy;
use acp_common::{
    rfc3339_from_unix, token_sha256, ShareDenyKind, ShareLedger, ShareStoreError,
    SHARE_FINGERPRINT_PREFIX,
};

use super::ShareService;
use crate::audit::AuditEvent;

/// 兑换结果（设计 §4）：NotMatched 沿用既有拒绝路径（peer-not-allowed），
/// 不留 share 审计；Storage 是持久化故障，fail-closed 拒绝。
#[derive(Debug)]
pub enum RedeemOutcome {
    Activated(PeerPolicy),
    NotMatched,
    Denied {
        share_id: String,
        kind: ShareDenyKind,
    },
    Storage(String),
}

#[derive(Debug, thiserror::Error)]
pub enum RevokeError {
    #[error("unknown share_id: {0}")]
    Unknown(String),
    #[error("share revoke persistence failed: {0}")]
    Store(#[from] ShareStoreError),
}

#[derive(Debug, Clone)]
pub struct RevokeReport {
    pub share_id: String,
    pub already_revoked: bool,
    pub policy_removed: bool,
}

impl ShareService {
    /// 兑换（设计 §4）：哈希匹配 + 状态判定 + 激活（activations+1、绑定 peer、
    /// 写策略表、审计 share-redeemed）。激活主序：台账先落盘，策略表失败回滚。
    pub fn redeem(&self, peer: &str, token: &str, now_secs: u64) -> RedeemOutcome {
        let hash = token_sha256(token);
        let mut state = self.lock();
        let Some(share_id) = state.find_id_by_token_hash(&hash) else {
            return RedeemOutcome::NotMatched;
        };
        let Some(kind) = state
            .get(&share_id)
            .and_then(|e| e.deny_kind(peer, now_secs))
        else {
            return self.activate(&mut state, &share_id, peer, now_secs);
        };
        self.audit.record(AuditEvent::ShareRedeemDenied {
            peer: peer.to_owned(),
            share_id: share_id.clone(),
            kind,
        });
        RedeemOutcome::Denied { share_id, kind }
    }

    fn activate(
        &self,
        state: &mut ShareLedger,
        share_id: &str,
        peer: &str,
        now_secs: u64,
    ) -> RedeemOutcome {
        let Some(entry) = state.get(share_id).cloned() else {
            return RedeemOutcome::NotMatched;
        };
        // grant 携带分享定向的工作区 id（多工作区加法）：jail 据此解析 cwd。
        let grant = PeerPolicy {
            scope: entry.scope,
            allow_mcp: entry.allow_mcp.clone(),
            ask_route: entry.ask_route,
            note: String::new(),
            granted_at: rfc3339_from_unix(now_secs),
            fingerprint: format!("{SHARE_FINGERPRINT_PREFIX}{share_id}"),
            workspace: entry.workspace.clone(),
        };
        let mut candidate = state.clone();
        let Some(c_entry) = candidate.get_mut(share_id) else {
            return RedeemOutcome::Storage("ledger entry vanished during redeem".to_owned());
        };
        c_entry.activations += 1;
        c_entry.bound_peer = Some(peer.to_owned());
        if let Err(err) = candidate.save(&self.ledger_path) {
            tracing::error!(peer, share_id, error = %err, "share ledger save failed on redeem");
            return RedeemOutcome::Storage(err.to_string());
        }
        let policy_err = {
            let mut table = self.policy.write().unwrap_or_else(|p| p.into_inner());
            table.grant(peer.to_owned(), grant.clone());
            table.save(&self.policy_path).err()
        };
        if let Some(err) = policy_err {
            tracing::error!(peer, share_id, error = %err, "policy save failed on redeem; rolling back ledger");
            if let Err(rollback) = state.save(&self.ledger_path) {
                tracing::error!(peer, share_id, error = %rollback, "share ledger rollback failed");
            }
            return RedeemOutcome::Storage(err.to_string());
        }
        *state = candidate;
        self.audit.record(AuditEvent::ShareRedeemed {
            peer: peer.to_owned(),
            share_id: share_id.to_owned(),
        });
        RedeemOutcome::Activated(grant)
    }

    /// 级联删除 share 来源策略条目（fingerprint 前缀识别）；非 share 来源不动。
    pub(super) fn remove_share_policy(
        &self,
        peer: &str,
        share_id: &str,
    ) -> Result<Option<acp_common::PeerPolicy>, ShareStoreError> {
        let mut table = self.policy.write().unwrap_or_else(|p| p.into_inner());
        let Some(existing) = table.lookup(peer) else {
            return Ok(None);
        };
        if !existing.fingerprint.starts_with(SHARE_FINGERPRINT_PREFIX) {
            tracing::warn!(
                peer,
                share_id,
                "policy entry not share-sourced; cascade skipped"
            );
            return Ok(None);
        }
        let removed = existing.clone();
        table.revoke(peer);
        if let Err(err) = table.save(&self.policy_path) {
            table.grant(peer.to_owned(), removed.clone());
            return Err(policy_store_err(err));
        }
        Ok(Some(removed))
    }

    /// 台账落盘失败时的策略回滚（best-effort，失败留 error 日志不静默）。
    pub(super) fn restore_share_policy(&self, peer: &str, policy: acp_common::PeerPolicy) {
        let mut table = self.policy.write().unwrap_or_else(|p| p.into_inner());
        table.grant(peer.to_owned(), policy);
        if let Err(err) = table.save(&self.policy_path) {
            tracing::error!(peer, error = %err, "policy restore after failed revoke failed");
        }
    }
}

/// PolicyTable::save 的错误类型归一到 ShareStoreError（save 只会是 IO 形态，
/// 其余变体不可达但显式收敛，不留隐式转换）。
fn policy_store_err(err: acp_common::PolicyStoreError) -> ShareStoreError {
    match err {
        acp_common::PolicyStoreError::Io(io) => ShareStoreError::Io(io),
        other => ShareStoreError::Io(std::io::Error::other(other.to_string())),
    }
}
