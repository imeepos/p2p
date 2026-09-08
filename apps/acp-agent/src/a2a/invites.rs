//! 邀请簿（docs/design/a2a-over-p2p-design.md §7.3 F3）：nonce 一次性持久化，
//! 落盘 <data-dir>/a2a-invites.json（0600，tmp+rename 原子写，与 grants 同款手法）。
//! 邀请生命周期：pending → accepted/rejected/expired。回执登记后写入 grants.json。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// 邀请簿文件名（相对 data_dir）。
pub const INVITES_FILE: &str = "a2a-invites.json";
/// 邀请条目上限：防写爆。
pub const INVITES_MAX: usize = 256;

/// 邀请状态。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InviteStatus {
    /// 等待回执。
    Pending,
    /// 已接受（回执已登记）。
    Accepted,
    /// 已拒绝。
    Rejected,
    /// 已过期（expiry < now）。
    Expired,
}

/// 单条邀请：nonce 为键（一次性）。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteEntry {
    /// 一次性 nonce（UUID v4 简化版）。
    pub nonce: String,
    /// agentId。
    pub agent_id: String,
    /// 宿主 PeerId（base58）。
    pub host_peer: String,
    /// 被邀请方 PeerId（base58）。
    pub invitee_peer: String,
    /// 邀请有效期截止（unix secs）。
    pub expiry: u64,
    /// 签发时刻（unix secs）。
    pub issued_at: u64,
    /// 状态。
    pub status: InviteStatus,
    /// 回执签名（base58，accepted 后填充）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_sig: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct InvitesFile {
    version: u32,
    invites: Vec<InviteEntry>,
}

const FILE_VERSION: u32 = 1;

/// 邀请簿：进程内 Mutex + 变更即原子落盘。
pub struct InviteStore {
    path: PathBuf,
    inner: Mutex<Vec<InviteEntry>>,
}

impl InviteStore {
    /// 打开（缺失=空簿）；损坏拒启（对齐 grants 纪律，禁止静默清空）。
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let invites = match std::fs::read(&path) {
            Ok(bytes) => {
                let file: InvitesFile = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("{} parse: {e}", path.display()))?;
                file.invites
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(err) => return Err(format!("{} read: {err}", path.display())),
        };
        Ok(Self {
            path,
            inner: Mutex::new(invites),
        })
    }

    /// 插入邀请（nonce 一次性：重复拒绝）。满员拒绝，失败留错误不静默。
    pub fn insert(&self, entry: InviteEntry) -> Result<(), String> {
        let mut invites = self.lock();
        if invites.iter().any(|i| i.nonce == entry.nonce) {
            return Err(format!("nonce already used: {}", entry.nonce));
        }
        if invites.len() >= INVITES_MAX {
            return Err(format!("invite store full ({INVITES_MAX})"));
        }
        invites.push(entry);
        self.persist(&invites)
    }

    /// 标记接受（回执登记）：nonce 必须存在且为 pending。
    pub fn accept(&self, nonce: &str, receipt_sig: &str) -> Result<(), String> {
        let mut invites = self.lock();
        let entry = invites
            .iter_mut()
            .find(|i| i.nonce == nonce)
            .ok_or_else(|| format!("nonce not found: {nonce}"))?;
        if entry.status != InviteStatus::Pending {
            return Err(format!("nonce {} not pending (status={:?})", nonce, entry.status));
        }
        entry.status = InviteStatus::Accepted;
        entry.receipt_sig = Some(receipt_sig.to_owned());
        self.persist(&invites)
    }

    /// 标记拒绝。
    pub fn reject(&self, nonce: &str) -> Result<(), String> {
        let mut invites = self.lock();
        let entry = invites
            .iter_mut()
            .find(|i| i.nonce == nonce)
            .ok_or_else(|| format!("nonce not found: {nonce}"))?;
        if entry.status != InviteStatus::Pending {
            return Err(format!("nonce {} not pending (status={:?})", nonce, entry.status));
        }
        entry.status = InviteStatus::Rejected;
        self.persist(&invites)
    }

    /// 撤销某 agent 全部 pending 邀请（agent 删除时调用）。
    pub fn revoke_agent(&self, agent_id: &str) -> Result<usize, String> {
        let mut invites = self.lock();
        let before = invites.len();
        invites.retain(|i| !(i.agent_id == agent_id && i.status == InviteStatus::Pending));
        let removed = before - invites.len();
        if removed > 0 {
            self.persist(&invites)?;
        }
        Ok(removed)
    }

    /// 查询 pending 邀请（按 invitee_peer 过滤）。
    pub fn pending_for_invitee(&self, invitee_peer: &str) -> Vec<InviteEntry> {
        self.lock()
            .iter()
            .filter(|i| i.invitee_peer == invitee_peer && i.status == InviteStatus::Pending)
            .cloned()
            .collect()
    }

    /// 查询已发送邀请（按 host_peer 过滤）。
    pub fn sent_by_host(&self, host_peer: &str) -> Vec<InviteEntry> {
        self.lock()
            .iter()
            .filter(|i| i.host_peer == host_peer)
            .cloned()
            .collect()
    }

    /// 查询 nonce 是否已使用（一次性检查）。
    pub fn is_nonce_used(&self, nonce: &str) -> bool {
        self.lock().iter().any(|i| i.nonce == nonce)
    }

    pub fn list(&self) -> Vec<InviteEntry> {
        self.lock().clone()
    }

    fn persist(&self, invites: &[InviteEntry]) -> Result<(), String> {
        let file = InvitesFile {
            version: FILE_VERSION,
            invites: invites.to_vec(),
        };
        let bytes = serde_json::to_vec_pretty(&file).map_err(|e| format!("encode: {e}"))?;
        write_atomic(&self.path, &bytes)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<InviteEntry>> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// tmp + rename 原子写（0600；对齐 grants.rs 同款纪律）。
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let tmp = path.with_extension("json.tmp");
    acp_common::write_private_file(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "a2a-invites-test-{tag}-{}.json",
            uuid::Uuid::new_v4().simple()
        ))
    }

    fn make_entry(nonce: &str, agent_id: &str, invitee_peer: &str) -> InviteEntry {
        InviteEntry {
            nonce: nonce.to_owned(),
            agent_id: agent_id.to_owned(),
            host_peer: "host-peer-123".to_owned(),
            invitee_peer: invitee_peer.to_owned(),
            expiry: 9999999999,
            issued_at: 1000,
            status: InviteStatus::Pending,
            receipt_sig: None,
        }
    }

    #[test]
    fn insert_and_nonce_check() {
        let path = temp_path("insert");
        let store = InviteStore::open(path.clone()).unwrap();
        assert!(!store.is_nonce_used("nonce-1"));
        store.insert(make_entry("nonce-1", "a1", "peer-1")).unwrap();
        assert!(store.is_nonce_used("nonce-1"));
        // 重复 nonce 拒绝
        assert!(store.insert(make_entry("nonce-1", "a1", "peer-2")).is_err());
        drop(store);
        let store = InviteStore::open(path.clone()).unwrap();
        assert!(store.is_nonce_used("nonce-1"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn accept_and_reject() {
        let path = temp_path("accept");
        let store = InviteStore::open(path.clone()).unwrap();
        store.insert(make_entry("n1", "a1", "p1")).unwrap();
        store.insert(make_entry("n2", "a1", "p2")).unwrap();
        
        store.accept("n1", "receipt-sig-123").unwrap();
        let list = store.list();
        let entry = list.iter().find(|i| i.nonce == "n1").unwrap();
        assert_eq!(entry.status, InviteStatus::Accepted);
        assert_eq!(entry.receipt_sig.as_deref(), Some("receipt-sig-123"));
        
        store.reject("n2").unwrap();
        let list = store.list();
        let entry = list.iter().find(|i| i.nonce == "n2").unwrap();
        assert_eq!(entry.status, InviteStatus::Rejected);
        
        // 重复 accept 拒绝
        assert!(store.accept("n1", "sig").is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn revoke_agent_removes_pending() {
        let path = temp_path("revoke");
        let store = InviteStore::open(path.clone()).unwrap();
        store.insert(make_entry("n1", "a1", "p1")).unwrap();
        store.insert(make_entry("n2", "a1", "p2")).unwrap();
        store.insert(make_entry("n3", "a2", "p1")).unwrap();
        store.accept("n1", "sig").unwrap();
        
        let removed = store.revoke_agent("a1").unwrap();
        assert_eq!(removed, 1, "only pending n2 removed, n1 already accepted");
        assert_eq!(store.list().len(), 2);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pending_for_invitee() {
        let path = temp_path("pending");
        let store = InviteStore::open(path.clone()).unwrap();
        store.insert(make_entry("n1", "a1", "p1")).unwrap();
        store.insert(make_entry("n2", "a1", "p1")).unwrap();
        store.insert(make_entry("n3", "a2", "p2")).unwrap();
        
        let pending = store.pending_for_invitee("p1");
        assert_eq!(pending.len(), 2);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn corrupt_store_rejects_open() {
        let path = temp_path("bad");
        std::fs::write(&path, b"{not json").unwrap();
        assert!(InviteStore::open(path.clone()).is_err());
        let _ = std::fs::remove_file(path);
    }
}