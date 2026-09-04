//! chat 命令域（CL3）：对齐 GUI 契约 §12 的 Tauri 命令语义。
//!
//! 数据面复用 crates/p2p-chat 门面（路径依赖，禁止复制实现）；--data-dir 与
//! GUI 同一目录约定：聊天库固定在 <data-dir>/chat，指向同一目录即同一份好友与历史。

pub mod context;
mod friend_update;
mod friends;
mod messages;
mod payload;
mod serve;

use std::fmt::Display;

use serde::Serialize;

use crate::error::{CliError, CliResult};

use friends::FriendsCommand;
use messages::{HistoryArgs, MediaCommand, SendArgs};
use serve::ServeArgs;

/// chat 域注册：friends/history/send/media（契约 §12）+ serve（E2E/守护支撑）。
#[derive(clap::Subcommand)]
pub enum ChatCommand {
    /// 好友簿：list / add / update / remove
    Friends {
        #[command(subcommand)]
        command: FriendsCommand,
    },
    /// 读与某对端的消息历史（time desc，limit 默认 50 上限 100）
    History(HistoryArgs),
    /// 发送消息（--text 文本 或 --file 附件）
    Send(SendArgs),
    /// 附件：file 查询落盘路径
    Media {
        #[command(subcommand)]
        command: MediaCommand,
    },
    /// 常驻运行聊天节点：输出 peerId 与监听地址后等待信号（E2E/守护支撑）
    Serve(ServeArgs),
}

pub async fn run(command: ChatCommand) -> CliResult<()> {
    match command {
        ChatCommand::Friends { command } => friends::run(command).await,
        ChatCommand::History(args) => messages::history(args).await,
        ChatCommand::Send(args) => messages::send(args).await,
        ChatCommand::Media { command } => messages::run_media(command).await,
        ChatCommand::Serve(args) => serve::run(args).await,
    }
}

/// chat 域输出：--json 单行紧凑 JSON（E2E 用 grep/sed 机械断言），文本模式给人读。
/// 有意不走 output::render（pretty 多行不便行级断言），框架文件保持 CL1 原样。
pub(crate) fn emit<T: Serialize>(json: bool, value: &T, text: &str) -> Result<(), CliError> {
    if json {
        let line = serde_json::to_string(value)
            .map_err(|e| CliError::Runtime(format!("JSON 序列化失败: {e}")))?;
        println!("{line}");
    } else {
        println!("{text}");
    }
    Ok(())
}

/// crate 中文错误 → CLI 运行失败（退出码 1）。
pub(crate) fn runtime_err<E: Display>(e: E) -> CliError {
    CliError::Runtime(e.to_string())
}