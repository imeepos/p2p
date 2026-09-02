//! p2p-cli：实验用命令行工具（design §12 调试工具、experiment-env E1-E3 执行载体）。
//!
//! 子命令：bootstrap / node / ping / discover。所有失败路径返回错误，
//! 由 main 统一打印到 stderr 并以非零退出码结束，禁止 panic。

pub mod bootstrap;
pub mod cli;
pub mod discover;
pub mod echo;
pub mod node;
pub mod ping;

use clap::Parser;

use crate::cli::{Cli, Command};

/// 分发子命令：成功返回进程退出码（通常 0），失败返回可读错误（main 转非零退出）。
pub async fn run() -> Result<i32, String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Bootstrap(args) => {
            bootstrap::run(args).await?;
            Ok(0)
        }
        Command::Node(args) => {
            node::run(args).await?;
            Ok(0)
        }
        Command::Ping(args) => {
            ping::run(args).await?;
            Ok(0)
        }
        Command::Discover(args) => {
            discover::run(args).await?;
            Ok(0)
        }
    }
}
