//! 子命令注册点：集中唯一。新增命令域 = 新增模块文件 + 在 Command 登记一个 variant。

use clap::Parser;

use crate::chat;
use crate::config;
use crate::identity;
use crate::node;
use crate::peer;
use crate::profile;

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

/// 命令域登记表：CL2 = node/config/profile/peer/identity；logs/update（CL4）。
#[derive(clap::Subcommand)]
pub enum Command {
    /// 节点域：状态/启停
    Node {
        #[command(subcommand)]
        command: node::NodeCommand,
    },
    /// 聊天域：friends/history/send/media（CL3）
    Chat {
        #[command(subcommand)]
        command: chat::ChatCommand,
    },
    /// 配置域：读取/保存
    Config {
        #[command(subcommand)]
        command: config::ConfigCommand,
    },
    /// 节点资料域：读取/保存
    Profile {
        #[command(subcommand)]
        command: profile::ProfileCommand,
    },
    /// 对端域：拨号/连接/挂断/测距
    Peer {
        #[command(subcommand)]
        command: peer::PeerCommand,
    },
    /// 身份域：重置
    Identity {
        #[command(subcommand)]
        command: identity::IdentityCommand,
    },
}