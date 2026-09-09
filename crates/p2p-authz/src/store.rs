//! 存储（authz-role-design §6）：<data>/authz/roles.json 与 bindings.json，
//! version=1 信封，tmp+rename 原子写（沿 acp-common PolicyStore 语义）。
//! 缺失文件 = 空表首用态；损坏/版本不符显式报错拒读，禁止静默回退空表
//! （§11 红线 2：authz 读失败 = 拒，绝不静默放行）。
//! roles.json 只落自定义角色；内建四角色为代码内闭集（§5），不落盘。
//! 存储测试见 [crate::store_tests]。

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::binding::Binding;
use crate::role::Role;

/// authz 数据子目录名（挂在 CLI data-dir 下）。
pub const AUTHZ_DIR: &str = "authz";
pub const ROLES_FILE: &str = "roles.json";
pub const BINDINGS_FILE: &str = "bindings.json";
/// 当前信封版本；读到其他版本 = 显式拒读（升级路径显式化）。
pub const FILE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct RolesEnvelope {
    version: u32,
    roles: Vec<Role>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BindingsEnvelope {
    version: u32,
    bindings: Vec<Binding>,
}

/// 存储错误：不可读 / 损坏 / 版本不符，三类显式区分。
#[derive(Debug, thiserror::Error)]
pub enum AuthzStoreError {
    #[error("authz 文件不可读: {0}")]
    Io(#[from] std::io::Error),
    #[error("authz 文件损坏，拒绝静默回退空表: {0}")]
    Corrupted(#[from] serde_json::Error),
    #[error("authz 文件版本 {0} 不支持（当前支持 version={FILE_VERSION}）")]
    UnsupportedVersion(u32),
}

pub fn roles_path(data_dir: &Path) -> PathBuf {
    data_dir.join(AUTHZ_DIR).join(ROLES_FILE)
}

pub fn bindings_path(data_dir: &Path) -> PathBuf {
    data_dir.join(AUTHZ_DIR).join(BINDINGS_FILE)
}

/// 读角色表：缺失 = 空（首用态）；损坏/版本不符显式报错。
pub fn load_roles(data_dir: &Path) -> Result<Vec<Role>, AuthzStoreError> {
    let path = roles_path(data_dir);
    let raw = match std::fs::read(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let envelope: RolesEnvelope = serde_json::from_slice(&raw)?;
    check_version(envelope.version)?;
    Ok(envelope.roles)
}

/// 读绑定表：语义同 [load_roles]。
pub fn load_bindings(data_dir: &Path) -> Result<Vec<Binding>, AuthzStoreError> {
    let path = bindings_path(data_dir);
    let raw = match std::fs::read(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let envelope: BindingsEnvelope = serde_json::from_slice(&raw)?;
    check_version(envelope.version)?;
    Ok(envelope.bindings)
}

/// 原子写角色表：同目录临时文件 + sync + rename；失败错误上抛并清理临时文件。
pub fn save_roles(data_dir: &Path, roles: &[Role]) -> Result<(), AuthzStoreError> {
    let envelope = RolesEnvelope {
        version: FILE_VERSION,
        roles: roles.to_vec(),
    };
    write_atomic(&roles_path(data_dir), &envelope)
}

/// 原子写绑定表：语义同 [save_roles]。
pub fn save_bindings(data_dir: &Path, bindings: &[Binding]) -> Result<(), AuthzStoreError> {
    let envelope = BindingsEnvelope {
        version: FILE_VERSION,
        bindings: bindings.to_vec(),
    };
    write_atomic(&bindings_path(data_dir), &envelope)
}

fn check_version(version: u32) -> Result<(), AuthzStoreError> {
    if version == FILE_VERSION {
        Ok(())
    } else {
        Err(AuthzStoreError::UnsupportedVersion(version))
    }
}

/// tmp + sync + rename 原子写：进程崩溃也不会留下半截正式文件；失败清理残留 .tmp。
fn write_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), AuthzStoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value)?;
    let tmp = path.with_extension("json.tmp");
    if let Err(e) = write_and_sync(&tmp, json.as_bytes()) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

fn write_and_sync(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
