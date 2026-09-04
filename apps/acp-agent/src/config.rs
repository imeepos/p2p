//! 桥配置：全部字段有默认值；配置文件（JSON）可省略任意字段，CLI 参数逐项覆盖。
//! 凭据不经配置传递：API key 只进子进程环境（设计 §6），本结构不含任何秘密。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use acp_common::consts::{PERMISSION_TIMEOUT_SECS, PROTOCOL_ID, REATTACH_WINDOW_DEFAULT_SECS};
use acp_common::AcpPaths;

pub const DEFAULT_DATA_DIR: &str = "./acp-data";
/// ready.agent 默认名（设计 §4.1 示例）。
pub const DEFAULT_AGENT_NAME: &str = "home-agent";
/// 连接总数默认上限（设计 §7：8 GB 节点建议 8 个并发控制台）。
pub const DEFAULT_MAX_CONNECTIONS: u32 = 8;
/// 客户端断流后子进程宽限期默认秒数。
pub const DEFAULT_GRACE_SECS: u64 = 10;
/// 子进程 stderr 滚动日志单文件上限与份数。
pub const CHILD_LOG_MAX_BYTES: u64 = 4 * 1024 * 1024;
pub const CHILD_LOG_MAX_FILES: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    /// 桥数据目录：策略表 / 子进程日志 / 节点身份都挂在其下。
    pub data_dir: String,
    /// 监听端口（0 = 随机）；facade 以端口为监听配置粒度。
    pub quic_port: u16,
    pub tcp_port: u16,
    /// 桥协议 ID，默认取 acp-common 常量（= p2p-relay proto_ids::ACP）。
    pub protocol_id: String,
    /// ready.agent 字段值。
    pub agent_name: String,
    /// 每连接专属子进程命令行（argv）。
    pub command: Vec<String>,
    /// 策略表路径；None = <data_dir>/acp-policy.json。
    pub policy_path: Option<String>,
    /// 子进程 stderr 滚动日志目录；None = <data_dir>/acp-logs。
    pub log_dir: Option<String>,
    /// 连接总数上限。
    pub max_connections: u32,
    /// 宽限期秒数。
    pub grace_secs: u64,
    /// sandbox 监狱根目录；None = <data_dir>/sandbox（设计 §6 工作区行）。
    pub sandbox_root: Option<String>,
    /// scope=workspace 的锁定授权目录；未配置则该 scope 拒绝 spawn。
    pub workspace_dir: Option<String>,
    /// 续连窗口秒数（设计 §5，默认取 acp-common 常量）。
    pub reattach_window_secs: u64,
    /// request_permission 客户端应答上限秒数，超时代答 reject-once（设计 §6）。
    pub permission_timeout_secs: u64,
    /// node 预定义 MCP 服务定义（名称 -> 完整定义；命令字节只在 host 手里）。
    pub mcp_definitions: BTreeMap<String, serde_json::Value>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            data_dir: DEFAULT_DATA_DIR.to_owned(),
            quic_port: 0,
            tcp_port: 0,
            protocol_id: PROTOCOL_ID.to_owned(),
            agent_name: DEFAULT_AGENT_NAME.to_owned(),
            command: default_command(),
            policy_path: None,
            log_dir: None,
            max_connections: DEFAULT_MAX_CONNECTIONS,
            grace_secs: DEFAULT_GRACE_SECS,
            sandbox_root: None,
            workspace_dir: None,
            reattach_window_secs: REATTACH_WINDOW_DEFAULT_SECS,
            permission_timeout_secs: PERMISSION_TIMEOUT_SECS,
            mcp_definitions: BTreeMap::new(),
        }
    }
}

fn default_command() -> Vec<String> {
    ["pnpm", "dsh", "--profile", "acp"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("read config {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("parse config {path}: {source}")]
    Json {
        path: String,
        source: serde_json::Error,
    },
    #[error("subprocess command must be non-empty argv")]
    EmptyCommand,
}

/// 读取 JSON 配置文件；字段可全部省略（serde(default)）。
pub fn load_file(path: &str) -> Result<AgentConfig, ConfigError> {
    let raw = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_owned(),
        source,
    })?;
    serde_json::from_str(&raw).map_err(|source| ConfigError::Json {
        path: path.to_owned(),
        source,
    })
}

impl AgentConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.command.is_empty() || self.command.iter().any(String::is_empty) {
            return Err(ConfigError::EmptyCommand);
        }
        Ok(())
    }

    pub fn paths(&self) -> AcpPaths {
        AcpPaths::new(&self.data_dir)
    }

    pub fn policy_path(&self) -> PathBuf {
        match &self.policy_path {
            Some(p) => PathBuf::from(p),
            None => self.paths().policy(),
        }
    }

    pub fn log_dir(&self) -> PathBuf {
        match &self.log_dir {
            Some(d) => PathBuf::from(d),
            None => self.paths().log_dir(),
        }
    }

    /// 宽限期下限 1s：0 会把退出阶梯退化成即时 SIGKILL。
    pub fn grace(&self) -> Duration {
        Duration::from_secs(self.grace_secs.max(1))
    }

    /// sandbox 监狱根：未配置时落在数据目录下。
    pub fn sandbox_root(&self) -> PathBuf {
        match &self.sandbox_root {
            Some(p) => PathBuf::from(p),
            None => self.paths().root.join("sandbox"),
        }
    }

    /// 续连窗口下限 1s：0 会让断流立即降级为退出阶梯。
    pub fn window(&self) -> Duration {
        Duration::from_secs(self.reattach_window_secs.max(1))
    }

    /// 权限应答上限下限 1s。
    pub fn permission_timeout(&self) -> Duration {
        Duration::from_secs(self.permission_timeout_secs.max(1))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_design() {
        let cfg = AgentConfig::default();
        assert_eq!(cfg.protocol_id, "/dsh-acp/1");
        assert_eq!(cfg.command, vec!["pnpm", "dsh", "--profile", "acp"]);
        assert_eq!(cfg.max_connections, 8);
        assert_eq!(cfg.grace_secs, 10);
        cfg.validate().expect("defaults must validate");
    }

    #[test]
    fn partial_file_fills_defaults() {
        let dir = std::env::temp_dir().join(format!("acp-agent-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("tmp dir");
        let path = dir.join("agent.json");
        let body = serde_json::json!({ "data_dir": "/tmp/x" }).to_string();
        std::fs::write(&path, body).expect("write");
        let cfg = load_file(path.to_str().expect("utf8")).expect("parse");
        assert_eq!(cfg.data_dir, "/tmp/x");
        assert_eq!(cfg.protocol_id, "/dsh-acp/1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_rejects_empty_command() {
        let mut cfg = AgentConfig::default();
        cfg.command.clear();
        assert!(cfg.validate().is_err());
    }
}
