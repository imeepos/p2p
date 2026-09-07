//! 工作区动态表（admin 管理面的权威数据源）：启动装配期从
//! <data-dir>/acp-workspaces.json 加载（缺失=以配置种子落盘初始化；损坏=拒启），
//! 运行期经 admin POST/DELETE 实时增删并原子持久化；jail cwd 解析与分享创建
//! 校验都改读本表，重启后即生效。legacy --workspace-dir 只作默认行兜底展示，
//! 不入文件、不可经 admin 删除。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::RwLock as StdRwLock;

use serde::{Deserialize, Serialize};

use crate::config::{WorkspaceDef, DEFAULT_WORKSPACE_ID};

/// 工作区表文件信封版本（升级路径显式）。
pub const WORKSPACES_FILE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct WorkspacesFile {
    version: u32,
    workspaces: Vec<WorkspaceDef>,
}

/// 工作区表错误：配置类错误映射 admin 4xx，存储类映射 5xx。
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceStoreError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("corrupt workspaces file {path}: {detail}")]
    Corrupt { path: String, detail: String },
    #[error("id/name/dir must be non-empty")]
    EmptyField,
    #[error("id must be 1-64 chars of [A-Za-z0-9_-]")]
    BadId,
    #[error("workspace id already exists: {0}")]
    DuplicateId(String),
    #[error("workspace not found: {0}")]
    Unknown(String),
    #[error("legacy default row is not manageable via admin")]
    LegacyDefault,
    #[error("dir must be an existing absolute directory: {0}")]
    InvalidDir(String),
}

#[derive(Debug)]
pub struct WorkspaceStore {
    entries: StdRwLock<Vec<WorkspaceDef>>,
    legacy_dir: Option<String>,
    path: PathBuf,
}

impl WorkspaceStore {
    /// 打开工作区表：缺失 = 以 (seed, legacy) 初始化并立刻落盘；
    /// 损坏/版本不符 = 显式报错拒启（禁止静默回退空表）。
    pub fn open(
        seed: &[WorkspaceDef],
        legacy_dir: Option<String>,
        path: PathBuf,
    ) -> Result<Self, WorkspaceStoreError> {
        let entries = match fs::read_to_string(&path) {
            Ok(raw) => {
                let file: WorkspacesFile =
                    serde_json::from_str(&raw).map_err(|err| WorkspaceStoreError::Corrupt {
                        path: path.display().to_string(),
                        detail: err.to_string(),
                    })?;
                if file.version != WORKSPACES_FILE_VERSION {
                    return Err(WorkspaceStoreError::Corrupt {
                        path: path.display().to_string(),
                        detail: format!("version {}", file.version),
                    });
                }
                file.workspaces
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                let seeded = seed.to_vec();
                let store = Self {
                    entries: StdRwLock::new(seeded.clone()),
                    legacy_dir,
                    path: path.clone(),
                };
                store.persist(&seeded)?;
                return Ok(store);
            }
            Err(err) => return Err(err.into()),
        };
        Ok(Self {
            entries: StdRwLock::new(entries),
            legacy_dir,
            path,
        })
    }

    /// 配置驱动的打开捷径：路径与种子都取自 AgentConfig（装配与测试共用）。
    pub fn open_for_config(
        config: &crate::config::AgentConfig,
    ) -> Result<Self, WorkspaceStoreError> {
        Self::open(
            &config.workspaces,
            config.workspace_dir.clone(),
            config.paths().workspaces(),
        )
    }

    /// 解析工作区行：id=None → 默认行（显式表项优先，回落 legacy 目录）。
    pub fn resolve(&self, id: Option<&str>) -> Option<WorkspaceDef> {
        let want = id.unwrap_or(DEFAULT_WORKSPACE_ID);
        let entries = self.lock();
        if let Some(hit) = entries.iter().find(|w| w.id == want) {
            return Some(hit.clone());
        }
        if want == DEFAULT_WORKSPACE_ID {
            return self.legacy_dir.as_ref().map(|dir| WorkspaceDef {
                id: DEFAULT_WORKSPACE_ID.to_owned(),
                name: DEFAULT_WORKSPACE_ID.to_owned(),
                dir: dir.clone(),
            });
        }
        None
    }

    /// 全列表（admin GET /workspaces）：显式表项 + legacy 兜底行（去重）。
    pub fn rows(&self) -> Vec<WorkspaceDef> {
        let mut rows = self.lock().clone();
        if let Some(fallback) = self.resolve(None) {
            if !rows.iter().any(|w| w.id == fallback.id) {
                rows.push(fallback);
            }
        }
        rows
    }

    /// 新增具名工作区：字段校验 + 目录存在性检查 + 原子持久化。
    pub fn add(
        &self,
        id: &str,
        name: &str,
        dir: &str,
    ) -> Result<WorkspaceDef, WorkspaceStoreError> {
        let id = id.trim();
        let name = name.trim();
        let dir = dir.trim();
        if id.is_empty() || name.is_empty() || dir.is_empty() {
            return Err(WorkspaceStoreError::EmptyField);
        }
        if id.len() > 64
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(WorkspaceStoreError::BadId);
        }
        if !Path::new(dir).is_absolute() || !Path::new(dir).is_dir() {
            return Err(WorkspaceStoreError::InvalidDir(dir.to_owned()));
        }
        let def = WorkspaceDef {
            id: id.to_owned(),
            name: name.to_owned(),
            dir: dir.to_owned(),
        };
        let mut entries = self.lock();
        if entries.iter().any(|w| w.id == def.id) {
            return Err(WorkspaceStoreError::DuplicateId(def.id.clone()));
        }
        entries.push(def.clone());
        if let Err(err) = self.persist(&entries) {
            entries.pop();
            return Err(err);
        }
        Ok(def)
    }

    /// 删除具名工作区：legacy 兜底行不可删；持久化失败回滚内存。
    pub fn remove(&self, id: &str) -> Result<(), WorkspaceStoreError> {
        let mut entries = self.lock();
        let Some(pos) = entries.iter().position(|w| w.id == id) else {
            if id == DEFAULT_WORKSPACE_ID && self.legacy_dir.is_some() {
                return Err(WorkspaceStoreError::LegacyDefault);
            }
            return Err(WorkspaceStoreError::Unknown(id.to_owned()));
        };
        let removed = entries.remove(pos);
        if let Err(err) = self.persist(&entries) {
            entries.push(removed);
            return Err(err);
        }
        Ok(())
    }

    fn lock(&self) -> std::sync::RwLockWriteGuard<'_, Vec<WorkspaceDef>> {
        self.entries
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// tmp+rename 原子落盘；写一半崩溃不留半截 JSON。
    fn persist(&self, entries: &[WorkspaceDef]) -> Result<(), WorkspaceStoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = WorkspacesFile {
            version: WORKSPACES_FILE_VERSION,
            workspaces: entries.to_vec(),
        };
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(&file)?)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}
