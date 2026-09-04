//! 节点策略表：PeerId → 授权模型（设计 §6）。默认拒绝：查无即拒。
//! 文件存取路径由调用方注入；tmp+rename 原子写；损坏显式报错，禁止静默回退空表。

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ErrorCode;

/// 工作区边界（设计 §6）：sandbox 每 peer 监狱 / workspace 锁定授权目录 / owner 全权。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Sandbox,
    Workspace,
    Owner,
}

/// request_permission 中 ask 的路由（设计 §6 工具行）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AskRoute {
    RemoteGui,
    OwnerLocal,
}

/// 单 peer 授权条目。granted_at 由调用方注入（RFC 3339），本库不持时钟依赖。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerPolicy {
    pub scope: Scope,
    /// mcpServers 白名单：按名引用 host 预定义服务，不在名单即剥离（§6 MCP 行）。
    #[serde(default)]
    pub allow_mcp: Vec<String>,
    pub ask_route: AskRoute,
    #[serde(default)]
    pub note: String,
    pub granted_at: String,
    /// TOFU 指纹确认面（§6 授权行）。
    pub fingerprint: String,
}

impl PeerPolicy {
    /// 白名单按名引用判定。
    pub fn mcp_allowed(&self, service_name: &str) -> bool {
        self.allow_mcp.iter().any(|name| name == service_name)
    }
}

/// 持久化信封：带版本号，升级路径显式。
#[derive(Debug, Serialize, Deserialize)]
struct PolicyFile {
    version: u32,
    peers: BTreeMap<String, PeerPolicy>,
}

const POLICY_FILE_VERSION: u32 = 1;

/// 策略表内存模型 + 文件存取。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyTable {
    peers: BTreeMap<String, PeerPolicy>,
}

impl PolicyTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// 默认拒绝：查无条目即 PeerNotAllowed（§12-Q2；审计只报码不泄细节，§12-Q5）。
    pub fn authorize(&self, peer_id: &str) -> Result<&PeerPolicy, ErrorCode> {
        self.peers.get(peer_id).ok_or(ErrorCode::PeerNotAllowed)
    }

    pub fn lookup(&self, peer_id: &str) -> Option<&PeerPolicy> {
        self.peers.get(peer_id)
    }

    pub fn grant(&mut self, peer_id: impl Into<String>, policy: PeerPolicy) {
        self.peers.insert(peer_id.into(), policy);
    }

    /// 撤销授权；返回是否确有条目。
    pub fn revoke(&mut self, peer_id: &str) -> bool {
        self.peers.remove(peer_id).is_some()
    }

    pub fn peers(&self) -> impl Iterator<Item = (&str, &PeerPolicy)> {
        self.peers.iter().map(|(id, policy)| (id.as_str(), policy))
    }

    /// 读取策略文件：缺失/损坏/版本不符一律显式报错——默认拒绝不等于静默吞存储故障。
    pub fn load(path: &Path) -> Result<Self, PolicyStoreError> {
        let raw = std::fs::read(path)?;
        let file: PolicyFile = serde_json::from_slice(&raw)?;
        if file.version != POLICY_FILE_VERSION {
            return Err(PolicyStoreError::UnsupportedVersion(file.version));
        }
        Ok(Self { peers: file.peers })
    }

    /// 原子写：先落同目录临时文件并 sync，再 rename 原子生效；失败错误上抛，
    /// 残留 .tmp 由下次 save 原名覆盖。
    pub fn save(&self, path: &Path) -> Result<(), PolicyStoreError> {
        let json = serde_json::to_string_pretty(&PolicyFile {
            version: POLICY_FILE_VERSION,
            peers: self.peers.clone(),
        })?;
        let tmp = path.with_extension("json.tmp");
        let mut file = File::create(&tmp)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PolicyStoreError {
    #[error("policy file unreadable: {0}")]
    Io(#[from] std::io::Error),
    #[error("policy file corrupted, refusing silent fallback: {0}")]
    Corrupted(#[from] serde_json::Error),
    #[error("policy file version {0} unsupported")]
    UnsupportedVersion(u32),
}
