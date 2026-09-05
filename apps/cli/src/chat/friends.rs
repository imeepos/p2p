//! chat friends 子域：list / add / update / remove / invites（契约 §12.1/§12.4）。
//!
//! add = 发好友邀请（邀请制：对方同意前不建好友，delivered 可判送达/挂起）；
//! remove 幂等：不在簿 removed=false 仍退出 0（对齐 GUI bool 返回语义）；
//! update 走 friend_update.rs 子域；invites 走 friend_invites.rs 子域；
//! --group 过滤与分组展示未分组恒置底。

use clap::{Args, Subcommand};
use p2p_chat::ChatFriend;
use serde::Serialize;

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;

use super::friend_update::UpdateArgs;
use super::{context, emit, runtime_err};

#[derive(Subcommand)]
pub enum FriendsCommand {
    /// 列出全部好友（--group 过滤；默认按分组展示，未分组置底）
    List(ListArgs),
    /// 发好友邀请（邀请制：对方同意后双向互为好友；重复邀请幂等刷新）
    Add(AddArgs),
    /// 更新好友（分组/昵称/备注补丁，至少提供一项；addrs 不可经此修改）
    Update(UpdateArgs),
    /// 移除好友（幂等；不在簿返回 removed=false）
    Remove(RemoveArgs),
    /// 邀请管理：list / accept / reject / cancel
    #[command(subcommand)]
    Invites(super::friend_invites::InvitesCommand),
}

#[derive(Args)]
pub struct ListArgs {
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 只显示该分组（空串 = 未分组；省略 = 全部按分组展示）
    #[arg(long)]
    group: Option<String>,
    /// 数据目录（聊天库在 <data-dir>/chat，与 GUI 同约定）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct AddArgs {
    /// 对端 peer id（base58，32 字节）
    peer_id: String,
    /// 显示名（trim 后 ≤64 字符；空串允许，由 crate 校验）
    #[arg(long, default_value = "")]
    nickname: String,
    /// 对端可拨地址（ip/u端口 或 ip/t端口；可重复）
    #[arg(long)]
    addr: Vec<String>,
    /// 备注（可选）
    #[arg(long)]
    note: Option<String>,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct RemoveArgs {
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
struct FriendRemoveReport {
    removed: bool,
}

pub async fn run(command: FriendsCommand) -> CliResult<()> {
    match command {
        FriendsCommand::List(args) => list(args).await,
        FriendsCommand::Add(args) => add(args).await,
        FriendsCommand::Update(args) => super::friend_update::run(args).await,
        FriendsCommand::Remove(args) => remove(args).await,
        FriendsCommand::Invites(command) => super::friend_invites::run(command).await,
    }
}

/// 分组归一读取：None/空串统一为 None（未分组），与落盘纪律同口径。
pub(crate) fn friend_group(f: &ChatFriend) -> Option<&str> {
    f.group.as_deref().filter(|s| !s.is_empty())
}

/// 分组聚合：组名字典序，未分组置底；组内保持好友簿原序。
fn group_sections(friends: &[ChatFriend]) -> Vec<(Option<&str>, Vec<&ChatFriend>)> {
    let mut order: Vec<Option<&str>> = Vec::new();
    for f in friends {
        let g = friend_group(f);
        if !order.contains(&g) {
            order.push(g);
        }
    }
    order.sort_by(|a, b| match (a, b) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    order
        .into_iter()
        .map(|g| (g, friends.iter().filter(|f| friend_group(f) == g).collect()))
        .collect()
}

async fn list(args: ListArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let mut friends = ctx.chat.friends_list().map_err(runtime_err)?;
    let filtered = args.group.is_some();
    if filtered {
        let want = args.group.as_deref().map(str::trim);
        let want = want.filter(|s| !s.is_empty());
        friends.retain(|f| friend_group(f) == want);
    }
    if friends.is_empty() {
        return emit(args.json, &friends, "好友簿为空（或该分组无成员）");
    }
    if filtered {
        let lines: Vec<String> = friends.iter().map(fmt_friend).collect();
        return emit(
            args.json,
            &friends,
            &format!("共 {} 位好友\n{}", friends.len(), lines.join("\n")),
        );
    }
    let mut text = format!("共 {} 位好友", friends.len());
    for (name, members) in group_sections(&friends) {
        text.push_str(&format!("\n[{}]", name.unwrap_or("未分组")));
        for m in members {
            text.push(' ');
            text.push_str(&fmt_friend(m));
        }
    }
    emit(args.json, &friends, &text)
}

async fn add(args: AddArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let report = ctx
        .chat
        .friend_invite(
            &args.peer_id,
            &args.nickname,
            args.addr.clone(),
            args.note.clone(),
        )
        .await
        .map_err(runtime_err)?;
    let state = if report.delivered {
        "已送达，等待对方同意"
    } else {
        "对端离线，邀请挂起（重连后自动重投）"
    };
    let text = format!(
        "已发送好友邀请 {}（{}）：{}",
        report.invite.nickname, report.invite.peer_id, state
    );
    emit(args.json, &report, &text)
}

async fn remove(args: RemoveArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let removed = ctx.chat.friend_remove(&args.peer_id).map_err(runtime_err)?;
    let text = if removed {
        format!("已移除好友 {}", args.peer_id)
    } else {
        format!("好友不在簿（幂等）: {}", args.peer_id)
    };
    emit(args.json, &FriendRemoveReport { removed }, &text)
}

fn fmt_friend(f: &ChatFriend) -> String {
    format!(
        "- {} {} addrs={:?} note={} group={}",
        f.nickname,
        f.peer_id,
        f.addrs,
        f.note.as_deref().unwrap_or("-"),
        friend_group(f).unwrap_or("-"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 好友簿条目（list/分组展示用）。
    fn friend(nickname: &str, group: Option<&str>) -> ChatFriend {
        ChatFriend {
            peer_id: "p".into(),
            nickname: nickname.into(),
            addrs: vec!["127.0.0.1/u1".into()],
            note: None,
            group: group.map(Into::into),
        }
    }

    /// 邀请簿条目（add=发邀请的 InviteReport 载荷）。
    fn fixture(direction: p2p_chat::InviteDirection) -> p2p_chat::FriendInvite {
        p2p_chat::FriendInvite {
            peer_id: "p".into(),
            nickname: "小 b".into(),
            addrs: vec!["127.0.0.1/u1".into()],
            note: None,
            direction,
            ts_ms: 1,
            delivered: false,
        }
    }

    #[test]
    fn invite_report_json_shape_is_judgeable() {
        let report = p2p_chat::InviteReport {
            delivered: true,
            invite: fixture(p2p_chat::InviteDirection::Out),
        };
        let v = serde_json::to_value(&report).unwrap();
        assert_eq!(v["delivered"], serde_json::json!(true));
        assert_eq!(v["invite"]["peerId"], serde_json::json!("p"));
        assert_eq!(v["invite"]["direction"], serde_json::json!("out"));
    }

    #[test]
    fn remove_report_json_shape() {
        let v = serde_json::to_value(FriendRemoveReport { removed: true }).unwrap();
        assert_eq!(v["removed"], serde_json::json!(true));
    }

    #[test]
    fn friend_text_line_contains_key_fields() {
        let line = fmt_friend(&friend("小 b", None));
        assert!(line.contains("小 b"));
        assert!(line.contains("127.0.0.1/u1"));
        assert!(line.contains("group=-"));
    }

    #[test]
    fn group_sections_orders_named_groups_with_ungrouped_last() {
        let friends = vec![
            friend("z", None),
            friend("b", Some("同事")),
            friend("a", Some("家人")),
            friend("c", Some("同事")),
        ];
        let sections = group_sections(&friends);
        let names: Vec<Option<&str>> = sections.iter().map(|(g, _)| *g).collect();
        assert_eq!(names, vec![Some("同事"), Some("家人"), None], "未分组置底");
        assert_eq!(sections[0].1.len(), 2, "组内保序聚合");
        // 空串组名视同未分组
        let blank = [friend("x", Some(""))];
        assert_eq!(friend_group(&blank[0]), None);
    }
}
