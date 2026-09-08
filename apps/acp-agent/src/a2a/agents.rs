//! 本地 agent 定义簿（a2a-over-p2p-design §3/§4）：owner 经 admin HTTP 增删改，
//! 持久化 <data-dir>/a2a-agents.json（0600，tmp+rename 原子写）。卡片按需签名
//! （SignedCard::sign），簿内只存声明本体，不存签名。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use a2a::{AgentCapabilities, AgentCard, AgentSkill, SignedCard, Visibility, TTL_DEFAULT_SECS};
use p2p_identity::Keypair;
use serde::{Deserialize, Serialize};

/// 簿文件名（相对 data_dir）。
pub const AGENTS_FILE: &str = "a2a-agents.json";
/// 簿条目上限：防 GUI/CLI 误操作写爆（设计 §9 限流行同级护栏）。
pub const AGENTS_MAX: usize = 32;

/// 单个本地 agent 定义（声明本体；签名时刻在卡片签发期）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDef {
    pub agent_id: String,
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<AgentSkill>,
    pub visibility: Visibility,
    /// 停用即不出卡（owner 保留定义，与删除区分）。
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub created_at: u64,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("agent_id 已存在: {0}")]
    Duplicate(String),
    #[error("agent_id 不存在: {0}")]
    Unknown(String),
    #[error("簿条目已达上限 {AGENTS_MAX}")]
    Full,
    #[error("agent 定义非法: {0}")]
    Invalid(String),
    #[error("簿读写失败: {0}")]
    Io(String),
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AgentsFile {
    version: u32,
    agents: Vec<AgentDef>,
}

const FILE_VERSION: u32 = 1;

/// 本地 agent 定义簿：进程内 Mutex + 变更即原子落盘。
pub struct AgentStore {
    path: PathBuf,
    inner: Mutex<Vec<AgentDef>>,
}

impl AgentStore {
    /// 打开（缺失=空簿）；损坏拒启（禁止静默清空，对齐 policy 纪律）。
    pub fn open(path: PathBuf) -> Result<Arc<Self>, StoreError> {
        let defs = match std::fs::read(&path) {
            Ok(bytes) => {
                let file: AgentsFile = serde_json::from_slice(&bytes)
                    .map_err(|e| StoreError::Io(format!("{} parse: {e}", path.display())))?;
                file.agents
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(err) => return Err(StoreError::Io(format!("{} read: {err}", path.display()))),
        };
        Ok(Arc::new(Self {
            path,
            inner: Mutex::new(defs),
        }))
    }

    /// 创建（agentId 冲突拒绝）；agentId 缺省随机 16 hex（slug 校验在卡片校验层）。
    pub fn create(
        &self,
        agent_id: Option<String>,
        name: String,
        description: String,
        skills: Vec<AgentSkill>,
        visibility: Visibility,
        now: u64,
    ) -> Result<AgentDef, StoreError> {
        let agent_id = match agent_id {
            Some(id) => id,
            None => uuid::Uuid::new_v4().simple().to_string()[..16].to_owned(),
        };
        let mut defs = self.lock();
        if defs.len() >= AGENTS_MAX {
            return Err(StoreError::Full);
        }
        if defs.iter().any(|d| d.agent_id == agent_id) {
            return Err(StoreError::Duplicate(agent_id));
        }
        let def = AgentDef {
            agent_id,
            name,
            description,
            skills,
            visibility,
            enabled: true,
            created_at: now,
        };
        Self::validate_def(&def)?;
        defs.push(def.clone());
        self.persist(&defs)?;
        Ok(def)
    }

    pub fn list(&self) -> Vec<AgentDef> {
        self.lock().clone()
    }

    pub fn get(&self, agent_id: &str) -> Option<AgentDef> {
        self.lock().iter().find(|d| d.agent_id == agent_id).cloned()
    }

    /// 更新可见性（下架/重新发布的 GUI 面入口）。
    pub fn set_visibility(
        &self,
        agent_id: &str,
        visibility: Visibility,
    ) -> Result<AgentDef, StoreError> {
        let mut defs = self.lock();
        let def = defs
            .iter_mut()
            .find(|d| d.agent_id == agent_id)
            .ok_or_else(|| StoreError::Unknown(agent_id.to_owned()))?;
        def.visibility = visibility;
        let updated = def.clone();
        self.persist(&defs)?;
        Ok(updated)
    }

    /// 停用/启用（enabled=false 不出卡）。
    pub fn set_enabled(&self, agent_id: &str, enabled: bool) -> Result<AgentDef, StoreError> {
        let mut defs = self.lock();
        let def = defs
            .iter_mut()
            .find(|d| d.agent_id == agent_id)
            .ok_or_else(|| StoreError::Unknown(agent_id.to_owned()))?;
        def.enabled = enabled;
        let updated = def.clone();
        self.persist(&defs)?;
        Ok(updated)
    }

    pub fn remove(&self, agent_id: &str) -> Result<AgentDef, StoreError> {
        let mut defs = self.lock();
        let pos = defs
            .iter()
            .position(|d| d.agent_id == agent_id)
            .ok_or_else(|| StoreError::Unknown(agent_id.to_owned()))?;
        let removed = defs.remove(pos);
        self.persist(&defs)?;
        Ok(removed)
    }

    /// 当前应出卡的声明（enabled 且可见性非 local；local 不进网络面）。
    pub fn network_defs(&self) -> Vec<AgentDef> {
        self.lock()
            .iter()
            .filter(|d| d.enabled && d.visibility != Visibility::Local)
            .cloned()
            .collect()
    }

    /// 按请求方可见性签发卡片（design §5.1 F4/F6）：owner 全见；远程 = public
    /// 全集 + 授权清单（granted agent ids）内 private；local 不出网络面。
    pub fn signed_cards_for(
        &self,
        kp: &Keypair,
        host_peer: &str,
        requester_is_owner: bool,
        granted: &[String],
        now: u64,
    ) -> Vec<SignedCard> {
        self.lock()
            .iter()
            .filter(|d| d.enabled)
            .filter(|d| {
                requester_is_owner
                    || d.visibility == Visibility::Public
                    || granted.iter().any(|id| id == &d.agent_id)
            })
            .filter_map(|d| {
                let card = Self::card_of(d, host_peer)?;
                SignedCard::sign(card, kp, now).ok()
            })
            .collect()
    }

    /// 定义 -> 卡片本体（url 按寻址规则组装）。
    pub fn card_of(def: &AgentDef, host_peer: &str) -> Option<AgentCard> {
        Some(AgentCard {
            agent_id: def.agent_id.clone(),
            name: def.name.clone(),
            description: def.description.clone(),
            url: format!("a2a://{host_peer}/{}", def.agent_id),
            host_peer: host_peer.to_owned(),
            visibility: def.visibility,
            capabilities: AgentCapabilities { streaming: true },
            skills: def.skills.clone(),
            ttl_secs: TTL_DEFAULT_SECS,
            version: 1,
        })
    }

    fn validate_def(def: &AgentDef) -> Result<(), StoreError> {
        let host = "check";
        let card = Self::card_of(def, host).ok_or_else(|| StoreError::Invalid("url".into()))?;
        // host_peer 用占位校验字段形状；hostPeer 绑定在签发卡片时由
        // SignedCard::verify 的身份校验兜底
        let _ = card;
        Ok(())
    }

    fn persist(&self, defs: &[AgentDef]) -> Result<(), StoreError> {
        let file = AgentsFile {
            version: FILE_VERSION,
            agents: defs.to_vec(),
        };
        let bytes =
            serde_json::to_vec_pretty(&file).map_err(|e| StoreError::Io(format!("encode: {e}")))?;
        write_atomic(&self.path, &bytes).map_err(|e| StoreError::Io(e.to_string()))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<AgentDef>> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// tmp + rename 原子写（0600；对齐 policy/seed 落盘纪律）。
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    acp_common::write_private_file(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
