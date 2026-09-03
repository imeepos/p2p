//! p2p-cli：实验用命令行工具（design §12 调试工具、experiment-env E1-E3 执行载体）。
//!
//! 子命令：bootstrap / node / ping / discover。所有失败路径返回错误，
//! 由 main 统一打印到 stderr 并按类别转退出码（见 [RunError]），禁止 panic。

pub mod bootstrap;
pub mod cli;
pub mod discover;
pub mod echo;
pub mod logging;
pub mod metrics_cmd;
pub mod metrics_log;
pub mod node;
pub mod ping;
pub mod relay_serve;

use std::fmt;

use clap::Parser;

use crate::cli::{parse_peer_id, parse_socket_addr, Cli, Command};

/// 正常退出（含 Ctrl-C 优雅退出）。
pub const EXIT_OK: i32 = 0;
/// 运行失败（节点装配、IO、协议等运行期错误）。
pub const EXIT_RUNTIME: i32 = 1;
/// 参数/配置错误（clap 用法错误同为 2）。
pub const EXIT_CONFIG: i32 = 2;

/// 运行期错误分类：main 据此映射可区分的退出码。
#[derive(Debug)]
pub enum RunError {
    /// 参数/配置错误（退出码 2）。
    Config(String),
    /// 运行失败（退出码 1）。
    Runtime(String),
}

impl RunError {
    pub fn exit_code(&self) -> i32 {
        match self {
            RunError::Config(_) => EXIT_CONFIG,
            RunError::Runtime(_) => EXIT_RUNTIME,
        }
    }

    fn message(&self) -> &str {
        match self {
            RunError::Config(m) | RunError::Runtime(m) => m,
        }
    }
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Config(_) => write!(f, "配置错误: {}", self.message()),
            RunError::Runtime(_) => write!(f, "运行失败: {}", self.message()),
        }
    }
}

/// 分发子命令：成功返回进程退出码，失败返回可读错误（main 转非零退出）。
///
/// 兼容入口：内部走 [run_with]，错误统一降级为字符串（原契约不变）。
pub async fn run() -> Result<i32, String> {
    let cli = Cli::parse();
    run_with(cli).await.map_err(|e| e.to_string())
}

/// 类型化入口：错误带类别，main 映射为可区分退出码（0/1/2）。
///
/// 配置类错误在分发前预校验（listen 地址 / PeerId），与子命令内部校验
/// 重复解析一次，换取退出码类别区分且不改子命令签名。
pub async fn run_with(cli: Cli) -> Result<i32, RunError> {
    match cli.command {
        Command::Bootstrap(args) => {
            ensure_addrs_valid(&[
                ("listen-quic", &args.listen_quic),
                ("listen-tcp", &args.listen_tcp),
            ])?;
            bootstrap::run(args).await.map_err(RunError::Runtime)?;
            Ok(EXIT_OK)
        }
        Command::Node(args) => {
            if let Some(q) = &args.listen_quic {
                ensure_addrs_valid(&[("listen-quic", q)])?;
            }
            node::run(args).await.map_err(RunError::Runtime)?;
            Ok(EXIT_OK)
        }
        Command::Ping(args) => {
            parse_peer_id(&args.peer_id).map_err(RunError::Config)?;
            ping::run(args).await.map_err(RunError::Runtime)?;
            Ok(EXIT_OK)
        }
        Command::Discover(args) => {
            discover::run(args).await.map_err(RunError::Runtime)?;
            Ok(EXIT_OK)
        }
        Command::Metrics(args) => {
            ensure_addrs_valid(&[
                ("listen-quic", &args.listen_quic),
                ("listen-tcp", &args.listen_tcp),
            ])?;
            metrics_cmd::run(args).await.map_err(RunError::Runtime)?;
            Ok(EXIT_OK)
        }
    }
}

fn ensure_addrs_valid(addrs: &[(&str, &str)]) -> Result<(), RunError> {
    for (name, value) in addrs {
        parse_socket_addr(value).map_err(|e| RunError::Config(format!("{name}: {e}")))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_error_exit_codes_are_distinguishable() {
        assert_eq!(RunError::Config("bad addr".into()).exit_code(), EXIT_CONFIG);
        assert_eq!(RunError::Config("bad addr".into()).exit_code(), 2);
        assert_eq!(RunError::Runtime("io".into()).exit_code(), EXIT_RUNTIME);
        assert_eq!(RunError::Runtime("io".into()).exit_code(), 1);
        assert_eq!(EXIT_OK, 0);
    }

    #[test]
    fn run_error_display_prefixes_category() {
        assert!(RunError::Config("x".into())
            .to_string()
            .starts_with("配置错误"));
        assert!(RunError::Runtime("y".into())
            .to_string()
            .starts_with("运行失败"));
    }

    #[test]
    fn ensure_addrs_valid_classifies_bad_addr_as_config() {
        let err = ensure_addrs_valid(&[("listen-quic", "no-colon")]).unwrap_err();
        assert!(matches!(err, RunError::Config(_)));
        assert!(ensure_addrs_valid(&[("listen-quic", "0.0.0.0:3400")]).is_ok());
    }
}
