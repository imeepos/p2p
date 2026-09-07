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
/// legacy --workspace-dir 隐式工作区 id（多工作区表项可显式占用同名）。
pub const DEFAULT_WORKSPACE_ID: &str = "default";

/// 具名工作区（多工作区分享的目标行；admin GET /workspaces 的条目形状）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceDef {
    /// 稳定 id：分享创建时定向（ShareSpec.workspace），jail 解析 cwd 的键。
    pub id: String,
    /// 展示名（GUI 列表行）。
    pub name: String,
    /// 锁定目录（绝对路径；jail 内 symlink 解析到真实目标）。
    pub dir: String,
}

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
    /// scope=workspace 的锁定授权目录（legacy 单工作区；None 且 workspaces 空则该 scope 拒绝）。
    pub workspace_dir: Option<String>,
    /// 多工作区表项（追加配置；GUI「分享工作区」列表数据源）。
    pub workspaces: Vec<WorkspaceDef>,
    /// 续连窗口秒数（设计 §5，默认取 acp-common 常量）。
    pub reattach_window_secs: u64,
    /// request_permission 客户端应答上限秒数，超时代答 reject-once（设计 §6）。
    pub permission_timeout_secs: u64,
    /// node 预定义 MCP 服务定义（名称 -> 完整定义；命令字节只在 host 手里）。
    pub mcp_definitions: BTreeMap<String, serde_json::Value>,
    /// MCP 定义文件路径（社区惯例为文件式 JSON 配置）；设置后以文件内容整体为准。
    pub mcp_definitions_path: Option<String>,
    /// 本地 admin HTTP 监听端口（0 = 随机，设计 §5）。
    pub admin_port: u16,
    /// 关闭本地 admin HTTP（设计 §5 --admin-disabled）。
    pub admin_disabled: bool,
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
            workspaces: Vec::new(),
            reattach_window_secs: REATTACH_WINDOW_DEFAULT_SECS,
            permission_timeout_secs: PERMISSION_TIMEOUT_SECS,
            mcp_definitions: BTreeMap::new(),
            mcp_definitions_path: None,
            admin_port: 0,
            admin_disabled: false,
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
    #[error("read mcp definitions {path}: {source}")]
    McpFileIo {
        path: String,
        source: std::io::Error,
    },
    #[error("parse mcp definitions {path}: {source}")]
    McpFileJson {
        path: String,
        source: serde_json::Error,
    },
    #[error("mcp definitions {path} must be a JSON object of name -> definition")]
    McpFileShape { path: String },
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

    /// 解析工作区行：id=None → 默认工作区（显式表项优先，回落 legacy workspace_dir）。
    pub fn workspace(&self, id: Option<&str>) -> Option<WorkspaceDef> {
        let want = id.unwrap_or(DEFAULT_WORKSPACE_ID);
        if let Some(hit) = self.workspaces.iter().find(|w| w.id == want) {
            return Some(hit.clone());
        }
        if want == DEFAULT_WORKSPACE_ID {
            return self.workspace_dir.as_ref().map(|dir| WorkspaceDef {
                id: DEFAULT_WORKSPACE_ID.to_owned(),
                name: DEFAULT_WORKSPACE_ID.to_owned(),
                dir: dir.clone(),
            });
        }
        None
    }

    /// 工作区全列表（admin GET /workspaces）：显式表项 + legacy 兜底行（去重）。
    pub fn workspace_rows(&self) -> Vec<WorkspaceDef> {
        let mut rows = self.workspaces.clone();
        if self.workspace(None).is_some() && !rows.iter().any(|w| w.id == DEFAULT_WORKSPACE_ID) {
            let fallback = self.workspace(None).expect("checked above");
            rows.push(fallback);
        }
        rows
    }

    /// 续连窗口下限 1s：0 会让断流立即降级为退出阶梯。
    pub fn window(&self) -> Duration {
        Duration::from_secs(self.reattach_window_secs.max(1))
    }

    /// 权限应答上限下限 1s。
    pub fn permission_timeout(&self) -> Duration {
        Duration::from_secs(self.permission_timeout_secs.max(1))
    }

    /// MCP 定义文件化加载：设置路径则以文件内容整体为准（确定性优先），
    /// 缺失/损坏/形状不对显式报错，禁止静默回退内嵌定义。
    pub fn load_mcp_definitions(&mut self) -> Result<(), ConfigError> {
        let Some(path) = self.mcp_definitions_path.clone() else {
            return Ok(());
        };
        let raw = std::fs::read_to_string(&path).map_err(|source| ConfigError::McpFileIo {
            path: path.clone(),
            source,
        })?;
        let root: serde_json::Value =
            serde_json::from_str(&raw).map_err(|source| ConfigError::McpFileJson {
                path: path.clone(),
                source,
            })?;
        let map = root
            .as_object()
            .ok_or(ConfigError::McpFileShape { path: path.clone() })?;
        self.mcp_definitions = map
            .iter()
            .map(|(name, definition)| (name.clone(), definition.clone()))
            .collect();
        Ok(())
    }
}
