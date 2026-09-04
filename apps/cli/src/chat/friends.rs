//! chat friends 子域：list / add / remove（契约 §12.1）。
//!
//! add 幂等（upsert，R3）：重复添加更新昵称/地址不报错，--json created 可判；
//! remove 幂等：不在簿 removed=false 仍退出 0（对齐 GUI bool 返回语义）。

use clap::{Args, Subcommand};
use p2p_chat::ChatFriend;
use serde::Serialize;

use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;

use super::{context, emit, runtime_err};

#[derive(Subcommand)]
pub enum FriendsCommand {
    /// 列出全部好友
    List(ListArgs),
    /// 添加好友（幂等 upsert；已在簿则为更新并标记 created=false）
    Add(AddArgs),
    /// 移除好友（幂等；不在簿返回 removed=false）
    Remove(RemoveArgs),
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

/// add 幂等报告：created=false 表示该 peer 已在簿（本次为更新）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FriendAddReport {
    created: bool,
    friend: ChatFriend,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FriendRemoveReport {
    removed: bool,
}

pub async fn run(command: FriendsCommand) -> CliResult {
    match command {
        FriendsCommand::List(args) => list(args).await,
        FriendsCommand::Add(args) => add(args).await,
        FriendsCommand::Remove(args) => remove(args).await,
    }
}

async fn list(args: ListArgs) -> CliResult {
    let ctx = context::open(&args.data_dir).await?;
    let friends = ctx.chat.friends_list().map_err(runtime_err)?;
    let text = if friends.is_empty() {
        "好友簿为空".to_string()
    } else {
        let lines: Vec<String> = friends.iter().map(fmt_friend).collect();
        format!("共 {} 位好友\n{}", friends.len(), lines.join("\n"))
    };
    emit(args.json, &friends, &text)
}

async fn add(args: AddArgs) -> CliResult {
    let ctx = context::open(&args.data_dir).await?;
    let existed = ctx
        .chat
        .friends_list()
        .map_err(runtime_err)?
        .iter()
        .any(|f| f.peer_id == args.peer_id);
    let friend = ctx
        .chat
        .friend_add(
            &args.peer_id,
            &args.nickname,
            args.addr.clone(),
            args.note.clone(),
        )
        .map_err(runtime_err)?;
    let report = FriendAddReport {
        created: !existed,
        friend,
    };
    let verb = if report.created {
        "已添加好友"
    } else {
        "好友已在簿，本次为更新"
    };
    emit(
        args.json,
        &report,
        &format!(
            "{} {}（{}）",
            verb, report.friend.nickname, report.friend.peer_id
        ),
    )
}

async fn remove(args: RemoveArgs) -> CliResult {
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
        "- {} {} addrs={:?} note={}",
        f.nickname,
        f.peer_id,
        f.addrs,
        f.note.as_deref().unwrap_or("-")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(nickname: &str) -> ChatFriend {
        ChatFriend {
            peer_id: "p".into(),
            nickname: nickname.into(),
            addrs: vec!["127.0.0.1/u1".into()],
            note: None,
        }
    }

    #[test]
    fn add_report_json_shape_is_judgeable() {
        let report = FriendAddReport {
            created: false,
            friend: fixture("b"),
        };
        let v = serde_json::to_value(&report).unwrap();
        assert_eq!(v["created"], serde_json::json!(false));
        assert_eq!(v["friend"]["peerId"], serde_json::json!("p"));
    }

    #[test]
    fn remove_report_json_shape() {
        let v = serde_json::to_value(FriendRemoveReport { removed: true }).unwrap();
        assert_eq!(v["removed"], serde_json::json!(true));
    }

    #[test]
    fn friend_text_line_contains_key_fields() {
        let line = fmt_friend(&fixture("小 b"));
        assert!(line.contains("小 b"));
        assert!(line.contains("127.0.0.1/u1"));
    }
}
