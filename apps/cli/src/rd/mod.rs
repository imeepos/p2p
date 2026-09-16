//! rd 命令域（M6）：headless 远程桌面运维面。host=前台常驻合成画面服务
//! （真实采集/注入由 GUI host 承担，合成源用于链路验收与开发）；probe=握手连通性探测。

pub mod host;
pub mod probe;

use clap::Subcommand;

use crate::error::CliResult;

/// rd 域命令面。
#[derive(Subcommand)]
pub enum RdCommand {
    /// host 前台常驻服务：进程活 = 受理开启，SIGINT/SIGTERM 优雅收口
    Host(host::HostArgs),
    /// viewer 握手探测：hello → hello_ack 即断（含 awaiting_approval 等拒绝原因）
    Probe(probe::ProbeArgs),
}

pub async fn run(cmd: RdCommand) -> CliResult<()> {
    match cmd {
        RdCommand::Host(args) => host::run(args).await,
        RdCommand::Probe(args) => probe::run(args).await,
    }
}