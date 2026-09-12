//! tunnel 命令域（gui-contract §19.8 预留条款落地，W-T5）：headless 被访侧
//! 前台进程面。本模块只做子命令分派；装配与运行时在 serve.rs。

use clap::Subcommand;

use crate::error::CliResult;

pub mod connect;
pub mod serve;

#[derive(Subcommand)]
pub enum TunnelCommand {
    /// 被访侧前台常驻服务：进程活 = 受理开启，SIGINT/SIGTERM 优雅收口
    Serve(serve::ServeArgs),
    /// 访侧前台常驻反代：headless 使用对端白名单内 127.0.0.1:<port> 服务
    Connect(connect::ConnectArgs),
}

pub async fn run(cmd: TunnelCommand) -> CliResult<()> {
    match cmd {
        TunnelCommand::Serve(args) => serve::run(args).await,
        TunnelCommand::Connect(args) => connect::run(args).await,
    }
}
