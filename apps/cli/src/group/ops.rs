//! group 群管理子域：create/list/invite/kick/leave/rename/disband（契约 §7）。
//!
//! 校验主体（成员 ⊆ 好友簿、群名、owner 权威）在 p2p-chat crate；本层只做 CLI 入参
//! 整形与输出。成员离线不失败：roster/kick/leave 经 goutbox 补投（design §6.2）。

use clap::Args;
use p2p_chat::GroupInfo;
use serde::Serialize;

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;

use crate::chat::{emit, runtime_err};
use crate::chat::context;

#[derive(Args)]
pub struct CreateArgs {
    /// 群名（trim 后 1..=64 字符）
    #[arg(long)]
    name: String,
    /// 初始成员 peer id（可重复；⊆ 好友簿且不含本机）
    #[arg(long = "member")]
    member: Vec<String>,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
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
pub struct InviteArgs {
    /// 目标群 id
    #[arg(long)]
    group: String,
    /// 受邀成员 peer id（可重复；⊆ 好友簿且不在群）
    #[arg(long = "member")]
    member: Vec<String>,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct KickArgs {
    /// 目标群 id
    #[arg(long)]
    group: String,
    /// 被移成员 peer id
    #[arg(long)]
    member: String,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct LeaveArgs {
    /// 目标群 id
    #[arg(long)]
    group: String,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct RenameArgs {
    /// 目标群 id
    #[arg(long)]
    group: String,
    /// 新群名（trim 后 1..=64 字符）
    #[arg(long)]
    name: String,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct DisbandArgs {
    /// 目标群 id
    #[arg(long)]
    group: String,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

/// 群信息输出行（文本模式的可读摘要）。
#[derive(Serialize)]
struct GroupLine<'a> {
    group_id: &'a str,
    name: &'a str,
    owner: &'a str,
    members: usize,
    rev: u64,
    state: String,
}

fn line(g: &GroupInfo) -> GroupLine<'_> {
    GroupLine {
        group_id: &g.group_id,
        name: &g.name,
        owner: &g.owner,
        members: g.members.len(),
        rev: g.rev,
        state: format!("{:?}", g.state).to_lowercase(),
    }
}

fn text_of(g: &GroupInfo) -> String {
    format!(
        "{} {} owner={} members={} rev={} state={}",
        g.group_id,
        g.name,
        g.owner,
        g.members.len(),
        g.rev,
        format!("{:?}", g.state).to_lowercase()
    )
}

pub(crate) async fn create(args: CreateArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let g = ctx
        .chat
        .group
        .group_create(&args.name, &args.member)
        .await
        .map_err(runtime_err)?;
    emit(args.json, &line(&g), &text_of(&g))
}

pub(crate) async fn list(args: ListArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let groups = ctx.chat.group.group_list();
    let lines: Vec<String> = groups.iter().map(|g| text_of(g)).collect();
    emit(args.json, &groups, &lines.join("\n"))
}

pub(crate) async fn invite(args: InviteArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let g = ctx
        .chat
        .group
        .group_invite(&args.group, &args.member)
        .await
        .map_err(runtime_err)?;
    emit(args.json, &line(&g), &text_of(&g))
}

pub(crate) async fn kick(args: KickArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let g = ctx
        .chat
        .group
        .group_kick(&args.group, &args.member)
        .await
        .map_err(runtime_err)?;
    emit(args.json, &line(&g), &text_of(&g))
}

pub(crate) async fn leave(args: LeaveArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let g = ctx
        .chat
        .group
        .group_leave(&args.group)
        .await
        .map_err(runtime_err)?;
    emit(args.json, &line(&g), &text_of(&g))
}

pub(crate) async fn rename(args: RenameArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let g = ctx
        .chat
        .group
        .group_rename(&args.group, &args.name)
        .await
        .map_err(runtime_err)?;
    emit(args.json, &line(&g), &text_of(&g))
}

pub(crate) async fn disband(args: DisbandArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let g = ctx
        .chat
        .group
        .group_disband(&args.group)
        .await
        .map_err(runtime_err)?;
    emit(args.json, &line(&g), &text_of(&g))
}