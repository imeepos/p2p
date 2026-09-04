//! cwd 监狱（设计 §6 工作区行）：scope 决定子进程工作目录，远程 peer 永远不可自指任意路径。
//! sandbox = <sandbox_root>/<peerId>/（每 peer 独立监狱，目录不存在则创建）；
//! workspace = 锁定授权目录（配置项，symlink 解析到真实目标后锁定）；
//! owner = 全 root（仅 loopback 场景，继承桥自身 cwd）。越界即拒绝 + 审计。

use std::path::{Path, PathBuf};

use acp_common::Scope;

use crate::config::AgentConfig;

#[derive(Debug, thiserror::Error)]
pub enum JailError {
    #[error("workspace scope granted but workspace_dir not configured")]
    WorkspaceUnconfigured,
    #[error("jail dir {path} unavailable: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("resolved cwd {resolved} escapes jail {jail}")]
    Escape { resolved: String, jail: String },
}

/// scope -> 子进程 cwd；None = 继承桥 cwd（owner 全 root，仅限 loopback 授予场景）。
pub fn resolve(
    cfg: &AgentConfig,
    scope: Scope,
    peer_id: &str,
) -> Result<Option<PathBuf>, JailError> {
    match scope {
        Scope::Owner => Ok(None),
        Scope::Sandbox => {
            let root = ensure_dir(&cfg.sandbox_root())?;
            let jail = ensure_dir(&root.join(sanitize(peer_id)))?;
            require_prefix(&jail, &root)?;
            Ok(Some(jail))
        }
        Scope::Workspace => {
            let dir = cfg
                .workspace_dir
                .as_ref()
                .ok_or(JailError::WorkspaceUnconfigured)?;
            Ok(Some(ensure_dir(Path::new(dir))?))
        }
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
        let cwd = resolve(&cfg, Scope::Sandbox, "peer/other").expect("sandbox jail");
        let cwd = cwd.expect("sandbox resolves to a path");
        assert!(cwd.ends_with("peer_other"));
        assert!(cwd.is_dir());
    }

    #[test]
    fn workspace_without_config_rejects() {
        let cfg = AgentConfig::default();
        let err = resolve(&cfg, Scope::Workspace, "peer").expect_err("must reject");
        assert!(matches!(err, JailError::WorkspaceUnconfigured));
    }

    #[test]
    fn workspace_locks_configured_dir() {
        let dir = std::env::temp_dir().join("jail-test-workspace");
        let cfg = AgentConfig {
            workspace_dir: Some(dir.to_string_lossy().into_owned()),
            ..AgentConfig::default()
        };
        let cwd = resolve(&cfg, Scope::Workspace, "peer").expect("workspace jail");
        assert_eq!(cwd.expect("path"), dir.canonicalize().expect("canonical"));
    }
}
