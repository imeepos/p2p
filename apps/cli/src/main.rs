//! p2pctl 入口：解析命令、分发命令域、错误转退出码（0 成功 / 1 运行失败 / 2 用法错误）。

mod acp;
mod chat;
mod cli;
mod config;
mod control;
mod daemon;
mod error;
mod gui;
mod identity;
mod lifecycle;
mod log;
mod metrics;
mod node;
mod ops;
mod output;
mod paths;
mod peer;
mod profile;
mod store;
mod types;
mod update;

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

async fn dispatch(cli: Cli) -> CliResult<()> {
    match cli.command {
        cli::Command::Node { command } => node::run(command).await,
        cli::Command::Chat { command } => chat::run(command).await,
        cli::Command::Config { command } => config::run(command).await,
        cli::Command::Profile { command } => profile::run(command).await,
        cli::Command::Peer { command } => peer::run(command).await,
        cli::Command::Gui { command } => gui::run(command).await,
        cli::Command::Identity { command } => identity::run(command).await,
        cli::Command::Log { command } => log::run(command).await,
        cli::Command::Metrics { command } => metrics::run(command).await,
        cli::Command::Update { command } => update::run(command).await,
        cli::Command::Acp { command } => acp::run(command).await,
    }
}