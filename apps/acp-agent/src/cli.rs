//! CLI 参数 + 可选配置文件：CLI 逐项覆盖文件值；全部有默认值。

use clap::Parser;

use crate::config::{load_file, AgentConfig, ConfigError};

#[derive(Debug, Parser)]
#[command(name = "acp-agent", about = "ACP over P2P agent 侧桥（/dsh-acp/1）")]
pub struct Cli {
    /// 可选 JSON 配置文件路径
    #[arg(long)]
    pub config: Option<String>,
    /// 桥数据目录
    #[arg(long)]
    pub data_dir: Option<String>,
    /// QUIC 监听端口（0=随机）
    #[arg(long)]
    pub quic_port: Option<u16>,
    /// TCP 监听端口（0=随机）
    #[arg(long)]
    pub tcp_port: Option<u16>,
    /// ready.agent 通告名
    #[arg(long)]
    pub agent_name: Option<String>,
    /// 子进程命令行（空格切分），默认 "pnpm dsh --profile acp"
    #[arg(long)]
    pub command: Option<String>,
    /// 策略表路径
    #[arg(long)]
    pub policy_path: Option<String>,
    /// 子进程 stderr 滚动日志目录
    #[arg(long)]
    pub log_dir: Option<String>,
    /// 连接总数上限
    #[arg(long)]
    pub max_connections: Option<u32>,
    /// 客户端断流后子进程宽限秒数
    #[arg(long)]
    pub grace_secs: Option<u64>,
    /// sandbox 监狱根目录
    #[arg(long)]
    pub sandbox_root: Option<String>,
    /// scope=workspace 锁定授权目录
    #[arg(long)]
    pub workspace_dir: Option<String>,
    /// 续连窗口秒数
    #[arg(long)]
    pub reattach_window_secs: Option<u64>,
    /// request_permission 应答上限秒数
    #[arg(long)]
    pub permission_timeout_secs: Option<u64>,
    /// MCP 定义文件路径（JSON object：名称 -> 服务定义）
    #[arg(long)]
    pub mcp_definitions_path: Option<String>,
}

pub fn assemble(cli: &Cli) -> Result<AgentConfig, ConfigError> {
    let mut cfg = match &cli.config {
        Some(path) => load_file(path)?,
        None => AgentConfig::default(),
    };
    if let Some(v) = &cli.data_dir {
        cfg.data_dir = v.clone();
    }
    if let Some(v) = cli.quic_port {
        cfg.quic_port = v;
    }
    if let Some(v) = cli.tcp_port {
        cfg.tcp_port = v;
    }
    if let Some(v) = &cli.agent_name {
        cfg.agent_name = v.clone();
    }
    if let Some(v) = &cli.command {
        cfg.command = v.split_whitespace().map(str::to_owned).collect();
    }
    if let Some(v) = &cli.policy_path {
        cfg.policy_path = Some(v.clone());
    }
    if let Some(v) = &cli.log_dir {
        cfg.log_dir = Some(v.clone());
    }
    if let Some(v) = cli.max_connections {
        cfg.max_connections = v;
    }
    if let Some(v) = cli.grace_secs {
        cfg.grace_secs = v;
    }
    if let Some(v) = &cli.sandbox_root {
        cfg.sandbox_root = Some(v.clone());
    }
    if let Some(v) = &cli.workspace_dir {
        cfg.workspace_dir = Some(v.clone());
    }
    if let Some(v) = cli.reattach_window_secs {
        cfg.reattach_window_secs = v;
    }
    if let Some(v) = cli.permission_timeout_secs {
        cfg.permission_timeout_secs = v;
    }
    if let Some(v) = &cli.mcp_definitions_path {
        cfg.mcp_definitions_path = Some(v.clone());
    }
    cfg.load_mcp_definitions()?;
    cfg.validate()?;
    Ok(cfg)
}
