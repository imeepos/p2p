//! p2pctl 入口：解析命令、分发命令域、错误转退出码（0 成功 / 1 运行失败 / 2 用法错误）。

mod chat;
mod cli;
mod error;
mod node;
mod output;

use clap::Parser;

use crate::cli::Cli;
use crate::error::CliResult;

#[tokio::main]
async fn main() {
    // 用法/参数错误由 clap 以退出码 2 处理，不进入 dispatch
    let cli = Cli::parse();
    if let Err(err) = dispatch(cli).await {
        eprintln!("p2pctl: {err}");
        std::process::exit(err.exit_code());
    }
}

async fn dispatch(cli: Cli) -> CliResult {
    match cli.command {
        cli::Command::Node { command } => node::run(command).await,
        cli::Command::Chat { command } => chat::run(command).await,
    }
}
