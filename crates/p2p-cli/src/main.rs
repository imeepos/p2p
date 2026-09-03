//! p2p-cli 可执行入口：装配日志，分发子命令，错误按类别转退出码。
//!
//! 退出码语义：0 正常（含 Ctrl-C 优雅退出）；1 运行失败；2 参数/配置错误
//! （clap 用法错误同样退出 2）。

use clap::Parser;

use p2p_cli::cli::Cli;
use p2p_cli::run_with;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    p2p_cli::logging::init_for(&cli.command);
    tracing::info!(target: "p2p_cli", command = ?cli.command, "cli startup");

    match run_with(cli).await {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("p2p-cli: {err}");
            std::process::exit(err.exit_code());
        }
    }
}
