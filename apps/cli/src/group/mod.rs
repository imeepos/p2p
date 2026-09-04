//! group 命令域（G4）：对齐 GUI 契约 §7 的 group_* Tauri 命令语义。
//!
//! 数据面复用 crates/p2p-chat 群门面（Chat.group，路径依赖禁止复制实现）；
//! --data-dir 与 GUI 同约定：聊天库在 <data-dir>/chat，群库在同根 groups.json。

pub mod ops;
pub mod send;

use clap::Subcommand;

use ops::{
    CreateArgs, DisbandArgs, InviteArgs, KickArgs, LeaveArgs, ListArgs, RenameArgs,
};

/// group 域注册：create/list/invite/kick/leave/rename/disband + send/history/media。
#[derive(Subcommand)]
pub enum GroupCommand {
    /// 建群（成员 ⊆ 好友簿、≤32、不含本机；群名 trim 1..=64）
    Create(CreateArgs),
    /// 列出全部群（含 left/kicked/disbanded）
    List(ListArgs),
    /// 邀请成员（owner-only；rev+1 推全体）
    Invite(InviteArgs),
    /// 移除成员（owner-only；rev+1 推余员 + G_KICK）
    Kick(KickArgs),
    /// 退群（本端 state=left；G_LEAVE 通知 owner）
    Leave(LeaveArgs),
    /// 改名（owner-only；rev+1 推 roster）
    Rename(RenameArgs),
    /// 解散（owner-only；对全体成员发 G_KICK(disbanded)）
    Disband(DisbandArgs),
    /// 发送群消息（--text 文本 或 --file 附件）
    Send(send::SendArgs),
    /// 读群历史（time desc，limit 默认 50 上限 100）
    History(send::HistoryArgs),
    /// 附件：file 查询落盘路径
    Media {
        #[command(subcommand)]
        command: send::MediaCommand,
    },
}

pub async fn run(command: GroupCommand) -> crate::error::CliResult<()> {
    match command {
        GroupCommand::Create(args) => ops::create(args).await,
        GroupCommand::List(args) => ops::list(args).await,
        GroupCommand::Invite(args) => ops::invite(args).await,
        GroupCommand::Kick(args) => ops::kick(args).await,
        GroupCommand::Leave(args) => ops::leave(args).await,
        GroupCommand::Rename(args) => ops::rename(args).await,
        GroupCommand::Disband(args) => ops::disband(args).await,
        GroupCommand::Send(args) => send::send(args).await,
        GroupCommand::History(args) => send::history(args).await,
        GroupCommand::Media { command } => send::run_media(command).await,
    }
}
