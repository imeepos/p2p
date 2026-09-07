//! 本机 agent 自描述文件（2026-09-07 用户裁决：GUI 免手填 admin token）：
//! acp-agent 启动装配 admin HTTP 后，把 管理地址+token+peer 写到用户级约定路径
//! ~/$HOME_DIR/acp/local-agent.json（0600），GUI 据此自动登记本机管理端点。
//! 红线沿 admin token：token 明文不进日志/审计；远端 agent 不落本文件，仍手动登记。
//! 纯库：home 根由调用方注入（user_home_dir 仅是给调用方的便利读法）。

use std::fs;
use std::io;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 用户主目录下的约定子目录（点前缀，跟随 unix 惯例）。
pub const DESCRIPTOR_SUBDIR: &str = ".dsh/acp";
/// 描述文件名。
pub const DESCRIPTOR_FILE: &str = "local-agent.json";
/// 文件信封版本。
pub const DESCRIPTOR_VERSION: u32 = 1;

/// 本机 agent 自描述（agent 写 / GUI 读的冻结契约）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalAgentDescriptor {
    pub version: u32,
    /// admin HTTP 地址，形如 http://127.0.0.1:<port>
    pub admin_url: String,
    /// admin Bearer token 原文（仅落本文件 0600，不进日志/审计）
    pub token: String,
    /// agent 节点 PeerId（base58）
    pub peer: String,
    /// agent 名（--agent-name，默认 home-agent）
    #[serde(default)]
    pub agent_name: String,
    /// 写入时刻（unix 秒；GUI 仅展示，不做新鲜度判定）
    pub written_at_unix: u64,
}

/// 读用户主目录（调用方注入约定的便利读法）：HOME 优先，Windows 回落 USERPROFILE。
pub fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// 约定路径推导（home 由调用方注入）：<home>/.dsh/acp/local-agent.json。
pub fn descriptor_path(home: &Path) -> PathBuf {
    home.join(DESCRIPTOR_SUBDIR).join(DESCRIPTOR_FILE)
}

/// 描述文件读写错误（缺失单列，GUI 据此静默回落手动登记）。
#[derive(Debug, thiserror::Error)]
pub enum DescriptorError {
    #[error("descriptor missing")]
    Missing,
    #[error("descriptor malformed: {0}")]
    Malformed(String),
    #[error("descriptor io: {0}")]
    Io(#[from] io::Error),
}

/// 写描述文件：tmp(0600)+rename 原子落盘，杜绝 GUI 读到半截 JSON。
pub fn write_descriptor(home: &Path, descriptor: &LocalAgentDescriptor) -> io::Result<PathBuf> {
    let dir = home.join(DESCRIPTOR_SUBDIR);
    fs::create_dir_all(&dir)?;
    let path = descriptor_path(home);
    let tmp = dir.join(format!("{}.tmp", DESCRIPTOR_FILE));
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(serde_json::to_vec_pretty(descriptor)?.as_slice())?;
        file.flush()?;
    }
    fs::rename(&tmp, &path)?;
    Ok(path)
}

/// 读描述文件：缺失/损坏显式报错，由调用方定夺（GUI 损坏时告警并回落）。
pub fn read_descriptor(path: &Path) -> Result<LocalAgentDescriptor, DescriptorError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Err(DescriptorError::Missing),
        Err(err) => return Err(err.into()),
    };
    serde_json::from_slice(&bytes).map_err(|err| DescriptorError::Malformed(err.to_string()))
}

/// 0600 私密写（admin token 文件与描述文件 tmp 共用；Windows 回落普通写）。
pub fn write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    #[cfg(unix)]
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(bytes)?;
        file.flush()
    }
    #[cfg(not(unix))]
    {
        fs::write(path, bytes)
    }
}
