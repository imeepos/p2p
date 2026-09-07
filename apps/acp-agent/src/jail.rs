//! cwd 监狱（设计 §6 工作区行）：scope 决定子进程工作目录，远程 peer 永远不可自指任意路径。
//! sandbox = <sandbox_root>/<peerId>/（每 peer 独立监狱，目录不存在则创建）；
//! workspace = 锁定授权目录（配置项，symlink 解析到真实目标后锁定）；
//! owner = 全 root（仅 loopback 场景，继承桥自身 cwd）。越界即拒绝 + 审计。

use std::path::{Path, PathBuf};

use acp_common::Scope;

use crate::config::AgentConfig;
use crate::workspaces::WorkspaceStore;

#[derive(Debug, thiserror::Error)]
pub enum JailError {
    #[error("workspace scope granted but workspace_dir not configured")]
    WorkspaceUnconfigured,
    #[error("workspace not configured: {id}")]
    WorkspaceUnknown { id: String },
    #[error("jail dir {path} unavailable: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("resolved cwd {resolved} escapes jail {jail}")]
    Escape { resolved: String, jail: String },
}

/// scope -> 子进程 cwd；None = 继承桥 cwd（owner 全 root，仅限 loopback 授予场景）。
/// workspace = 分享定向的工作区 id（None = 默认工作区；未知 id 显式拒绝）。
pub fn resolve(
    cfg: &AgentConfig,
    workspaces: &WorkspaceStore,
    scope: Scope,
    peer_id: &str,
    workspace: Option<&str>,
) -> Result<Option<PathBuf>, JailError> {
    match scope {
        Scope::Owner => Ok(None),
        Scope::Sandbox => {
            let root = ensure_dir(&cfg.sandbox_root())?;
            let jail = ensure_dir(&root.join(sanitize(peer_id)))?;
            require_prefix(&jail, &root)?;
            Ok(Some(jail))
        }
        Scope::Workspace => match workspaces.resolve(workspace) {
            None => Err(match workspace {
                Some(id) => JailError::WorkspaceUnknown { id: id.to_owned() },
                None => JailError::WorkspaceUnconfigured,
            }),
            Some(ws) => Ok(Some(ensure_dir(Path::new(&ws.dir))?)),
        },
    }
}

/// 前缀必须是组件级包含（Path::starts_with 语义），字符串前缀会放行兄弟目录。
fn require_prefix(resolved: &Path, jail: &Path) -> Result<(), JailError> {
    if resolved.starts_with(jail) {
        Ok(())
    } else {
        Err(JailError::Escape {
            resolved: resolved.display().to_string(),
            jail: jail.display().to_string(),
        })
    }
}

fn ensure_dir(path: &Path) -> Result<PathBuf, JailError> {
    std::fs::create_dir_all(path).map_err(|source| JailError::Io {
        path: path.display().to_string(),
        source,
    })?;
    path.canonicalize().map_err(|source| JailError::Io {
        path: path.display().to_string(),
        source,
    })
}

/// peer id 白名单字符化：非 [A-Za-z0-9_-] 一律替换为下划线，杜绝路径段注入。
fn sanitize(peer_id: &str) -> String {
    peer_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_neutralizes_path_segments() {
        assert_eq!(sanitize("../etc/passwd"), "___etc_passwd");
        assert_eq!(sanitize("abc-XYZ_123"), "abc-XYZ_123");
    }

    #[test]
    fn prefix_is_component_wise() {
        let base = std::env::temp_dir().join("jail-test-prefix");
        let inside = base.join("peer").join("sub");
        let sibling = base.join("peer-twin");
        assert!(require_prefix(&inside, &base).is_ok());
        assert!(require_prefix(&sibling, &base.join("peer")).is_err());
    }

    #[test]
    fn sandbox_creates_per_peer_jail() {
        let cfg = AgentConfig {
            sandbox_root: Some(
                std::env::temp_dir()
                    .join("jail-test-sandbox")
                    .to_string_lossy()
                    .into_owned(),
            ),
            ..AgentConfig::default()
        };
        let ws = empty_store(&cfg, "sandbox");
        let cwd = resolve(&cfg, &ws, Scope::Sandbox, "peer/other", None).expect("sandbox jail");
        let cwd = cwd.expect("sandbox resolves to a path");
        assert!(cwd.ends_with("peer_other"));
        assert!(cwd.is_dir());
    }

    #[test]
    fn workspace_without_config_rejects() {
        let cfg = AgentConfig::default();
        let ws = empty_store(&cfg, "reject");
        let err = resolve(&cfg, &ws, Scope::Workspace, "peer", None).expect_err("must reject");
        assert!(matches!(err, JailError::WorkspaceUnconfigured));
    }

    #[test]
    fn workspace_locks_configured_dir() {
        let dir = std::env::temp_dir().join("jail-test-workspace");
        let cfg = AgentConfig {
            workspace_dir: Some(dir.to_string_lossy().into_owned()),
            ..AgentConfig::default()
        };
        let ws = empty_store(&cfg, "legacy");
        let cwd = resolve(&cfg, &ws, Scope::Workspace, "peer", None).expect("workspace jail");
        assert_eq!(cwd.expect("path"), dir.canonicalize().expect("canonical"));
    }

    #[test]
    fn workspace_by_id_locks_named_dir() {
        let dir = std::env::temp_dir().join("jail-test-ws-named");
        let cfg = AgentConfig {
            workspaces: vec![crate::WorkspaceDef {
                id: "ws1".to_owned(),
                name: "named".to_owned(),
                dir: dir.to_string_lossy().into_owned(),
            }],
            ..AgentConfig::default()
        };
        let ws = crate::workspaces::WorkspaceStore::open_for_config(&cfg).expect("ws store");
        let cwd = resolve(&cfg, &ws, Scope::Workspace, "peer", Some("ws1")).expect("named jail");
        assert_eq!(cwd.expect("path"), dir.canonicalize().expect("canonical"));
        let err =
            resolve(&cfg, &ws, Scope::Workspace, "peer", Some("nope")).expect_err("must reject");
        assert!(matches!(err, JailError::WorkspaceUnknown { id } if id == "nope"));
    }

    /// 空表存储（legacy 兜底沿用 cfg.workspace_dir；落盘到独立临时目录）。
    fn empty_store(cfg: &AgentConfig, tag: &str) -> WorkspaceStore {
        let dir = std::env::temp_dir().join(format!("jail-test-ws-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tmp dir");
        WorkspaceStore::open(
            &cfg.workspaces,
            cfg.workspace_dir.clone(),
            dir.join("ws.json"),
        )
        .expect("ws store")
    }
}
