//! 子命令注册点：集中唯一。新增命令域 = 新增模块文件 + 在 Command 登记一个 variant。

use clap::Parser;

use crate::node;

#[derive(Parser)]
#[command(
    name = "p2pctl",
    version,
    about = "p2p 对等 CLI：GUI 命令面的等价命令行入口"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// 命令域登记表：config/profile/peer/identity（CL2）、chat（CL3）、logs/update（CL4）。
#[derive(clap::Subcommand)]
pub enum Command {
    /// 节点域：状态/启停
    Node {
        #[command(subcommand)]
        command: node::NodeCommand,
    },
}
