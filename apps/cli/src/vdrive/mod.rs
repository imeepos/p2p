//! vdrive 命令域：网络硬盘 headless 前台进程面。serve = 被挂端（把本地
//! 目录经 /vdrive/fs/1 供全网访问），mount = 挂载端（本机 WebDAV 桥 +
//! 可选自动挂载，OS 直接挂载）。进程活 = 服务开启，SIGINT/SIGTERM 收口。

use clap::Subcommand;

use crate::error::CliResult;

pub mod common;
pub mod mount;
pub mod serve;

#[derive(Subcommand)]
pub enum VDriveCommand {
    /// 被挂端前台常驻：本地目录 → /vdrive/fs/1（--root 必须存在）
    Serve(serve::ServeArgs),
    /// 挂载端前台常驻：远端 Peer 目录 → 本机 WebDAV 桥（--mount 自动挂载）
    Mount(mount::MountArgs),
}

pub async fn run(cmd: VDriveCommand) -> CliResult<()> {
    match cmd {
        VDriveCommand::Serve(args) => serve::run(args).await,
        VDriveCommand::Mount(args) => mount::run(args).await,
    }
}
