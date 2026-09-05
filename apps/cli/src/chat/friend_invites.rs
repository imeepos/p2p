//! chat friends invites 子域（邀请制加好友）：list / accept / reject / cancel。
//! 语义对齐契约 §12.4：同意前不建好友；同意后双向互为好友；全部 --json 单行可断言。

use clap::{Args, Subcommand};
use p2p_chat::ChatFriend;
use serde::Serialize;

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;

use super::{context, emit, runtime_err};

#[derive(Subcommand)]
pub enum InvitesCommand {
    /// 列出邀请（out = 待对方同意；in = 待本机处理）
    List(ListArgs),
    /// 同意来邀（本机立即互为好友，并回投 ACCEPT 完成对端建簿）
    Accept(AcceptArgs),
    /// 拒绝来邀（通知对方尽力而为）
    Reject(RejectArgs),
    /// 撤回本机待同意邀请
    Cancel(RejectArgs),
}

#[derive(Args)]
pub struct ListArgs {
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录（聊天库在 <data-dir>/chat，与 GUI 同约定）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct AcceptArgs {
    /// 邀请方 peer id
    peer_id: String,
    /// 本机为对方设置的显示名（缺省回退 PeerId 缩略，与 update --nickname 空串同口径）
    #[arg(long, default_value = "")]
    nickname: String,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct RejectArgs {
    /// 对端 peer id
    peer_id: String,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InviteOpReport {
    ok: bool,
}

pub async fn run(command: InvitesCommand) -> CliResult<()> {
    match command {
        InvitesCommand::List(args) => list(args).await,
        InvitesCommand::Accept(args) => accept(args).await,
        InvitesCommand::Reject(args) => reject(args).await,
        InvitesCommand::Cancel(args) => cancel(args).await,
    }
}

async fn list(args: ListArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let invites = ctx.chat.invites_list().map_err(runtime_err)?;
    if invites.is_empty() {
        return emit(args.json, &invites, "无待处理邀请");
    }
    let mut text = format!("共 {} 条邀请", invites.len());
    for i in &invites {
        let dir = match i.direction {
            p2p_chat::InviteDirection::Out => "待对方同意",
            p2p_chat::InviteDirection::In => "待本机处理",
        };
        let state = if i.delivered { "已送达" } else { "未送达" };
        text.push_str(&format!(
            "
- {} {}（{dir}，{state}）",
            i.nickname, i.peer_id
        ));
    }
    emit(args.json, &invites, &text)
}

async fn accept(args: AcceptArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let friend: ChatFriend = ctx
        .chat
        .invite_accept(&args.peer_id, &args.nickname)
        .await
        .map_err(runtime_err)?;
    emit(
        args.json,
        &InviteOpReport { ok: true },
        &format!("已同意好友邀请：{}（{}）", friend.nickname, friend.peer_id),
    )
}

async fn reject(args: RejectArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    ctx.chat.invite_reject(&args.peer_id).await.map_err(runtime_err)?;
    emit(
        args.json,
        &InviteOpReport { ok: true },
        &format!("已拒绝好友邀请: {}", args.peer_id),
    )
}

async fn cancel(args: RejectArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let cancelled = ctx.chat.invite_cancel(&args.peer_id).await.map_err(runtime_err)?;
    let text = if cancelled {
        format!("已撤回好友邀请: {}", args.peer_id)
    } else {
        format!("无待同意邀请（幂等）: {}", args.peer_id)
    };
    emit(args.json, &InviteOpReport { ok: cancelled }, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_report_json_shape() {
        let v = serde_json::to_value(InviteOpReport { ok: true }).unwrap();
        assert_eq!(v["ok"], serde_json::json!(true));
    }
}
