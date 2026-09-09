//! 分享台账服务（设计 §3/§4/§5/§10）：创建/兑换/撤销/列表。
//! 兑换是握手授权瀑布第二级：激活绑定 PeerId、写 share 前缀指纹的策略条目、
//! 自动绑定 authz 内建角色 guest（authz-role-design §8 准入双查的绑定侧，
//! A2 回归修复）、审计 share-redeemed；状态拒绝走 ShareDenyKind（码 + 四条审计键）。
//! 持久化顺序：台账先落盘，策略表写失败即回滚台账（两阶段间隙收敛到最小）。

pub mod admin;
pub mod api;
pub mod redeem;

pub use redeem::{RedeemOutcome, RevokeError, RevokeReport};

#[cfg(test)]
mod admin_cors_tests;
#[cfg(test)]
mod admin_tests;
#[cfg(test)]
mod admin_ws_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_authz_bind;
#[cfg(test)]
mod tests_redeem;
#[cfg(test)]
mod testutil;

use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, RwLock as StdRwLock};

use acp_common::{generate_token, Scope, ShareEntry, ShareLedger, ShareSpec, ShareStoreError};
use uuid::Uuid;

use crate::audit::AuditSink;
use crate::config::AgentConfig;
use crate::workspaces::WorkspaceStore;

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
    /// 工作区动态表（与 SessionDeps 同源；admin 增删后分享校验即时生效）。
    workspaces: Arc<WorkspaceStore>,
    /// 桥数据根（authz 自动绑定落 <data-dir>/authz/，与 DiskAuthz 同源）。
    data_dir: PathBuf,
}

/// share 兑换自动创建绑定的 note 标记：revoke 级联只回收带此标记的绑定，
/// 人工绑定（更严者为准）不动。
pub(crate) const SHARE_AUTO_BIND_NOTE: &str = "share auto-bind guest";

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
    #[error("workspace not configured: {0}")]
    WorkspaceUnknown(String),
    #[error("share ledger write failed: {0}")]
    Store(#[from] ShareStoreError),
}

impl ShareService {
    /// 打开台账：缺失 = 首启空账（留告警）；损坏/版本不符 = 显式报错拒启。
    pub fn open(
        config: &AgentConfig,
        workspaces: Arc<WorkspaceStore>,
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
            workspaces,
            data_dir: PathBuf::from(&config.data_dir),
        })
    }

    /// 创建分享（设计 §5 POST /shares 语义）。Owner 不可经分享产生（设计 §3）；
    /// scope=workspace 未配 workspace-dir 即拒（设计 §11-Q5，fail-closed 前移）；
    /// 定向工作区不存在同样创建即拒（多工作区加法）。
    pub fn create(&self, spec: ShareSpec, now_secs: u64) -> Result<ShareCreated, ShareCreateError> {
        if spec.scope == Scope::Owner {
            return Err(ShareCreateError::OwnerScope);
        }
        if spec.scope == Scope::Workspace {
            match self.workspaces.resolve(spec.workspace.as_deref()) {
                None if spec.workspace.is_none() => {
                    return Err(ShareCreateError::WorkspaceUnconfigured);
                }
                None => {
                    return Err(ShareCreateError::WorkspaceUnknown(
                        spec.workspace.unwrap_or_default(),
                    ));
                }
                Some(_) => {}
            }
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
        // 级联回收兑换时自动创建的 authz 绑定（防其他域残留授权面）；
        // 失败不阻塞 revoke：无策略条目准入第一闸已拒，绑定残留无害，留错误信号。
        if let Some(peer) = &bound_peer {
            if removed.is_some() {
                self.remove_auto_binding(peer);
            }
        }
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
    fn lock(&self) -> MutexGuard<'_, ShareLedger> {
        self.ledger
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
