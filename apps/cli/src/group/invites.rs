//! group invites 子域（同意制入群邀请，IMC2）：send / list / accept / reject。
//! 语义对齐 GUI 契约 §11 与 p2p-chat 门面：owner-only 发起、受邀者决策；
//! 对端离线不失败（delivered=false 挂起，重连重投，文本与 JSON 双形态可观察）；
//! --json 单行紧凑可断言（口径同 chat 域 emit）。

use clap::{Args, Subcommand};
use p2p_chat::GroupInvite;

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;

use crate::chat::{context, emit, runtime_err};

#[derive(Subcommand)]
pub enum InvitesCommand {
    /// 发起入群邀请（owner-only；对端离线挂起 delivered=false 待重投）
    Send(SendArgs),
    /// 列出邀请（in=待本机处理，out=待对方同意；tsMs 倒序）
    List(ListArgs),
    /// 同意入群（owner 离线时决策挂起，重连/重启重投）
    Accept(AcceptArgs),
    /// 拒绝入群（--reason 随决策帧回送，可省略）
    Reject(RejectArgs),
}

#[derive(Args)]
pub struct SendArgs {
    /// 目标群 id（须为本机 owned active 群）
    #[arg(long)]
    group: String,
    /// 受邀人 peer id（须在好友簿且不在群）
    #[arg(long)]
    peer: String,
    /// 邀请附言（可省略）
    #[arg(long)]
    note: Option<String>,
    /// 邀请人展示名（缺省回退 PeerId 缩略，同 chat friends add 口径）
    #[arg(long, default_value = "")]
    nickname: String,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录（聊天库在 <data-dir>/chat，与 GUI 同约定）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct ListArgs {
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct AcceptArgs {
    /// 邀请条目 id（group invites list 可见）
    invite_id: String,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct RejectArgs {
    /// 邀请条目 id（group invites list 可见）
    invite_id: String,
    /// 拒绝理由（随决策帧回送对方，可省略）
    #[arg(long)]
    reason: Option<String>,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

/// 送达观察信号：发送/决策帧是否本轮送达；false = 离线挂起（显式可判，非静默）。
fn delivery_note(delivered: bool) -> &'static str {
    if delivered {
        "已送达"
    } else {
        "未送达：对端离线，已挂起待重连重投"
    }
}

/// 文本模式条目行：方向/状态/送达三信号齐备（人读可判，机器走 --json）。
fn invite_line(i: &GroupInvite) -> String {
    let dir = match i.direction {
        p2p_chat::GroupInviteDirection::Out => "out",
        p2p_chat::GroupInviteDirection::In => "in",
    };
    let state = match i.state {
        p2p_chat::GroupInviteState::Pending => "pending",
        p2p_chat::GroupInviteState::Accepted => "accepted",
        p2p_chat::GroupInviteState::Rejected => "rejected",
    };
    format!(
        "- {} 群[{}] {} -> {} 方向={} 状态={} {}",
        i.id,
        i.group_name,
        i.inviter,
        i.invitee,
        dir,
        state,
        delivery_note(i.delivered)
    )
}

pub async fn run(command: InvitesCommand) -> CliResult<()> {
    match command {
        InvitesCommand::Send(args) => send(args).await,
        InvitesCommand::List(args) => list(args).await,
        InvitesCommand::Accept(args) => accept(args).await,
        InvitesCommand::Reject(args) => reject(args).await,
    }
}

async fn send(args: SendArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let report = ctx
        .chat
        .group
        .group_invite_member(&args.group, &args.peer, &args.nickname, args.note)
        .await
        .map_err(runtime_err)?;
    let text = format!(
        "已发起入群邀请 {}：群[{}] -> {}（{}）",
        report.invite.id,
        report.invite.group_name,
        report.invite.invitee,
        delivery_note(report.delivered)
    );
    emit(args.json, &report, &text)
}

async fn list(args: ListArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let invites = ctx.chat.group.group_invites_list().map_err(runtime_err)?;
    if invites.is_empty() {
        return emit(args.json, &invites, "无入群邀请");
    }
    let mut text = format!("共 {} 条入群邀请", invites.len());
    for i in &invites {
        text.push_str(&format!("\n{}", invite_line(i)));
    }
    emit(args.json, &invites, &text)
}

async fn accept(args: AcceptArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let invite = ctx
        .chat
        .group
        .group_invite_accept(&args.invite_id)
        .await
        .map_err(runtime_err)?;
    emit(
        args.json,
        &invite,
        &format!(
            "已同意入群邀请 {}（群[{}]，owner 离线时挂起重投）",
            invite.id, invite.group_name
        ),
    )
}

async fn reject(args: RejectArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let invite = ctx
        .chat
        .group
        .group_invite_reject(&args.invite_id, args.reason)
        .await
        .map_err(runtime_err)?;
    emit(
        args.json,
        &invite,
        &format!("已拒绝入群邀请 {}（群[{}]）", invite.id, invite.group_name),
    )
}

#[cfg(test)]
mod tests;
