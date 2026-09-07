//! 分享台账服务（设计 §3/§4/§5/§10）：创建/兑换/撤销/列表。
//! 兑换是握手授权瀑布第二级：激活绑定 PeerId、写 share 前缀指纹的策略条目、
//! 审计 share-redeemed；状态拒绝走 ShareDenyKind（码 + 四条审计键）。
//! 持久化顺序：台账先落盘，策略表写失败即回滚台账（两阶段间隙收敛到最小）。

pub mod admin;
pub mod api;

#[cfg(test)]
mod admin_cors_tests;
#[cfg(test)]
mod admin_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_redeem;
#[cfg(test)]
mod testutil;

use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, RwLock as StdRwLock};

use acp_common::policy::PeerPolicy;
use acp_common::{
    generate_token, rfc3339_from_unix, token_sha256, Scope, ShareDenyKind, ShareEntry, ShareLedger,
    ShareSpec, ShareStoreError, SHARE_FINGERPRINT_PREFIX,
};
use uuid::Uuid;

use crate::audit::{AuditEvent, AuditSink};
use crate::config::AgentConfig;

/// 链接组装要素（设计 §5）：本机 PeerId 与对外监听地址（含中继），
/// 由节点既有自省面取得，main 装配期注入。
#[derive(Debug, Clone)]
pub struct LinkContext {
    pub peer: String,
    pub addrs: Vec<String>,
}

pub struct ShareService {
    ledger: Mutex<ShareLedger>,
    ledger_path: PathBuf,
    policy_path: PathBuf,
    policy: Arc<StdRwLock<acp_common::PolicyTable>>,
    audit: Arc<dyn AuditSink>,
    workspace_ready: bool,
}

/// 创建结果：token 原文只在返回值出现一次（进创建响应与链接，禁止落盘）。
#[derive(Debug, Clone)]
pub struct ShareCreated {
    pub entry: ShareEntry,
    pub token: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ShareCreateError {
    #[error("owner scope can never be produced via share")]
    OwnerScope,
    #[error("workspace scope share requires --workspace-dir configured")]
    WorkspaceUnconfigured,
    #[error("share ledger write failed: {0}")]
    Store(#[from] ShareStoreError),
}

#[derive(Debug, Clone)]
pub struct RevokeReport {
    pub share_id: String,
    pub already_revoked: bool,
    pub policy_removed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum RevokeError {
    #[error("unknown share_id: {0}")]
    Unknown(String),
    #[error("share revoke persistence failed: {0}")]
    Store(#[from] ShareStoreError),
}

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

impl ShareService {
    /// 打开台账：缺失 = 首启空账（留告警）；损坏/版本不符 = 显式报错拒启。
    pub fn open(
        config: &AgentConfig,
        policy: Arc<StdRwLock<acp_common::PolicyTable>>,
        audit: Arc<dyn AuditSink>,
    ) -> Result<Self, ShareStoreError> {
        let path = config.paths().shares();
        let ledger = match ShareLedger::load(&path) {
            Ok(ledger) => ledger,
            Err(ShareStoreError::Io(err)) if err.kind() == ErrorKind::NotFound => {
                tracing::warn!(path = %path.display(), "share ledger absent; starting empty");
                ShareLedger::new()
            }
            Err(err) => return Err(err),
        };
        Ok(Self {
            ledger: Mutex::new(ledger),
            ledger_path: path,
            policy_path: config.policy_path(),
            policy,
            audit,
            workspace_ready: config.workspace_dir.is_some(),
        })
    }

    /// 创建分享（设计 §5 POST /shares 语义）。Owner 不可经分享产生（设计 §3）；
    /// scope=workspace 未配 workspace-dir 即拒（设计 §11-Q5，fail-closed 前移）。
    pub fn create(&self, spec: ShareSpec, now_secs: u64) -> Result<ShareCreated, ShareCreateError> {
        if spec.scope == Scope::Owner {
            return Err(ShareCreateError::OwnerScope);
        }
        if spec.scope == Scope::Workspace && !self.workspace_ready {
            return Err(ShareCreateError::WorkspaceUnconfigured);
        }
        let token = generate_token();
        let entry = ShareEntry::new(spec, &token, now_secs, Uuid::new_v4());
        let mut state = self.lock();
        let mut candidate = state.clone();
        candidate.insert(entry.clone());
        candidate
            .save(&self.ledger_path)
            .map_err(ShareCreateError::Store)?;
        *state = candidate;
        Ok(ShareCreated { entry, token })
    }

    /// 脱敏列表（BTreeMap 序）：调用方仍不得输出 token 原文与哈希。
    pub fn list(&self) -> Vec<ShareEntry> {
        self.lock().iter().map(|(_, entry)| entry.clone()).collect()
    }

    /// 撤销（设计 §3）：置 revoked；已绑定 peer 则级联删除 share 来源策略条目。
    pub fn revoke(&self, share_id: &str) -> Result<RevokeReport, RevokeError> {
        let mut state = self.lock();
        let Some(entry) = state.get(share_id) else {
            return Err(RevokeError::Unknown(share_id.to_owned()));
        };
        let already_revoked = entry.revoked;
        let bound_peer = entry.bound_peer.clone();
        let mut candidate = state.clone();
        let Some(c_entry) = candidate.get_mut(share_id) else {
            return Err(RevokeError::Unknown(share_id.to_owned()));
        };
        c_entry.revoked = true;
        // 先断策略（切断访问面），台账落盘失败则把条目原样放回。
        let removed = match &bound_peer {
            Some(peer) => self.remove_share_policy(peer, share_id)?,
            None => None,
        };
        if let Err(err) = candidate.save(&self.ledger_path) {
            if let (Some(peer), Some(policy)) = (&bound_peer, &removed) {
                self.restore_share_policy(peer, policy.clone());
            }
            return Err(RevokeError::Store(err));
        }
        *state = candidate;
        Ok(RevokeReport {
            share_id: share_id.to_owned(),
            already_revoked,
            policy_removed: removed.is_some(),
        })
    }

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
        let grant = PeerPolicy {
            scope: entry.scope,
            allow_mcp: entry.allow_mcp.clone(),
            ask_route: entry.ask_route,
            note: String::new(),
            granted_at: rfc3339_from_unix(now_secs),
            fingerprint: format!("{SHARE_FINGERPRINT_PREFIX}{share_id}"),
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
    fn remove_share_policy(
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
    fn restore_share_policy(&self, peer: &str, policy: acp_common::PeerPolicy) {
        let mut table = self.policy.write().unwrap_or_else(|p| p.into_inner());
        table.grant(peer.to_owned(), policy);
        if let Err(err) = table.save(&self.policy_path) {
            tracing::error!(peer, error = %err, "policy restore after failed revoke failed");
        }
    }

    fn lock(&self) -> MutexGuard<'_, ShareLedger> {
        self.ledger
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
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
