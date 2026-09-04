//! 子命令注册点：集中唯一。新增命令域 = 新增模块文件 + 在 Command 登记一个 variant。

use clap::Parser;

use crate::acp;
use crate::chat;
use crate::config;
use crate::gui;
use crate::identity;
use crate::log;
use crate::metrics;
use crate::node;
use crate::peer;
use crate::profile;
use crate::update;

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

/// 命令域登记表：CL2 = node/config/profile/peer/identity；CL3 = chat；CL4 = log/metrics/update。
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
    /// GUI 域：控制通道原语 status/screenshot/record/navigate/invoke（GC2）
    Gui {
        #[command(subcommand)]
        command: gui::GuiCommand,
    },
    /// 身份域：重置
    Identity {
        #[command(subcommand)]
        command: identity::IdentityCommand,
    },
    /// 日志域：GUI 前端日志 tail/path/clear（CL4）
    Log {
        #[command(subcommand)]
        command: log::LogCommand,
    },
    /// 指标域：运行时指标快照（CL4 补齐 metrics_get 对等）
    Metrics {
        #[command(subcommand)]
        command: metrics::MetricsCommand,
    },
    /// 更新域：release 检查与发布页 URL 输出（CL4）
    Update {
        #[command(subcommand)]
        command: update::UpdateCommand,
    },
    /// ACP 域：节点主人策略管理 allow/deny/list（ACP5，设计 §3/§6）
    Acp {
        #[command(subcommand)]
        command: acp::AcpCommand,
    },
}
