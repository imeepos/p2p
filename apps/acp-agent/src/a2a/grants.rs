//! 私有 agent 授权清单（a2a-over-p2p-design §6/§9）：owner 授权的 (agentId, peer)
//! 集合，落盘 <data-dir>/a2a-grants.json（0600，tmp+rename 原子写）。默认拒绝：
//! 不在清单内的远程 peer 对 private agent 不可见也不可聊。A2A5 邀请回执登记
//! 接入本簿；撤销传播（card/remove 推送）由 admin 面承担。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// 授权簿文件名（相对 data_dir）。
pub const GRANTS_FILE: &str = "a2a-grants.json";
/// 清单条目上限：防邀请放大授权写爆（设计 §9 F3）。
pub const GRANTS_MAX: usize = 256;

/// 单条授权：agentId × peer 二元组。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantEntry {
    pub agent_id: String,
    pub peer: String,
    pub granted_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct GrantsFile {
    version: u32,
    grants: Vec<GrantEntry>,
}

const FILE_VERSION: u32 = 1;

/// 授权簿：进程内 Mutex + 变更即原子落盘。
pub struct GrantStore {
    path: PathBuf,
    inner: Mutex<Vec<GrantEntry>>,
}

impl GrantStore {
    /// 打开（缺失=空簿）；损坏拒启（对齐 policy/agents 纪律，禁止静默清空）。
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let grants = match std::fs::read(&path) {
            Ok(bytes) => {
                let file: GrantsFile = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("{} parse: {e}", path.display()))?;
                file.grants
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(err) => return Err(format!("{} read: {err}", path.display())),
        };
        Ok(Self {
            path,
            inner: Mutex::new(grants),
        })
    }

    /// 授权（重复授权幂等成功）。满员拒绝，失败留错误不静默。
    pub fn grant(&self, agent_id: &str, peer: &str, now: u64) -> Result<(), String> {
        let mut grants = self.lock();
        if grants
            .iter()
            .any(|g| g.agent_id == agent_id && g.peer == peer)
        {
            return Ok(());
        }
        if grants.len() >= GRANTS_MAX {
            return Err(format!("grant store full ({GRANTS_MAX})"));
        }
        grants.push(GrantEntry {
            agent_id: agent_id.to_owned(),
            peer: peer.to_owned(),
            granted_at: now,
        });
        self.persist(&grants)
    }

    /// 撤销单条；返回是否确有删除（撤销传播方据此决定是否广播）。
    pub fn revoke(&self, agent_id: &str, peer: &str) -> Result<bool, String> {
        let mut grants = self.lock();
        let before = grants.len();
        grants.retain(|g| !(g.agent_id == agent_id && g.peer == peer));
        let removed = grants.len() != before;
        if removed {
            self.persist(&grants)?;
        }
        Ok(removed)
    }

    /// 撤销某 agent 全部授权（agent 删除时调用）。
    pub fn revoke_agent(&self, agent_id: &str) -> Result<usize, String> {
        let mut grants = self.lock();
        let before = grants.len();
        grants.retain(|g| g.agent_id != agent_id);
        let removed = before - grants.len();
        if removed > 0 {
            self.persist(&grants)?;
        }
        Ok(removed)
    }

    pub fn is_granted(&self, agent_id: &str, peer: &str) -> bool {
        self.lock()
            .iter()
            .any(|g| g.agent_id == agent_id && g.peer == peer)
    }

    /// 某 agent 的授权 peer 集（撤销传播 / card 相可见性过滤用）。
    pub fn peers_for(&self, agent_id: &str) -> HashSet<String> {
        self.lock()
            .iter()
            .filter(|g| g.agent_id == agent_id)
            .map(|g| g.peer.clone())
            .collect()
    }

    /// 某 peer 被授权的 agent 集（card 相可见性 / task 相门禁共用）。
    pub fn agents_for_peer(&self, peer: &str) -> Vec<String> {
        self.lock()
            .iter()
            .filter(|g| g.peer == peer)
            .map(|g| g.agent_id.clone())
            .collect()
    }

    pub fn list(&self) -> Vec<GrantEntry> {
        self.lock().clone()
    }

    fn persist(&self, grants: &[GrantEntry]) -> Result<(), String> {
        let file = GrantsFile {
            version: FILE_VERSION,
            grants: grants.to_vec(),
        };
        let bytes = serde_json::to_vec_pretty(&file).map_err(|e| format!("encode: {e}"))?;
        write_atomic(&self.path, &bytes)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<GrantEntry>> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// tmp + rename 原子写（0600；对齐 agents.rs 同款纪律）。
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
            "a2a-grants-test-{tag}-{}.json",
            uuid::Uuid::new_v4().simple()
        ))
    }

    #[test]
    fn grant_is_granted_roundtrip_and_persist() {
        let path = temp_path("rt");
        let store = GrantStore::open(path.clone()).unwrap();
        assert!(!store.is_granted("a1", "peer-x"));
        store.grant("a1", "peer-x", 100).unwrap();
        store.grant("a1", "peer-x", 200).unwrap(); // 幂等
        assert!(store.is_granted("a1", "peer-x"));
        assert!(!store.is_granted("a1", "peer-y"));
        assert_eq!(store.list().len(), 1);
        drop(store);
        let store = GrantStore::open(path.clone()).unwrap();
        assert!(store.is_granted("a1", "peer-x"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn revoke_single_and_whole_agent() {
        let path = temp_path("rv");
        let store = GrantStore::open(path.clone()).unwrap();
        store.grant("a1", "p1", 1).unwrap();
        store.grant("a1", "p2", 1).unwrap();
        store.grant("a2", "p1", 1).unwrap();
        assert!(store.revoke("a1", "p1").unwrap());
        assert!(!store.revoke("a1", "p1").unwrap(), "重复撤销返回 false");
        assert_eq!(store.peers_for("a1").len(), 1);
        assert_eq!(store.revoke_agent("a1").unwrap(), 1);
        assert!(store.peers_for("a1").is_empty());
        assert_eq!(store.list().len(), 1, "a2 授权不受影响");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn corrupt_store_rejects_open() {
        let path = temp_path("bad");
        std::fs::write(&path, b"{not json").unwrap();
        assert!(GrantStore::open(path.clone()).is_err());
        let _ = std::fs::remove_file(path);
    }
}
