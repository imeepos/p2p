# -*- coding: utf-8 -*-
# 一次性补丁脚本（任务完成后删除）：CLI friends add 邀请化 + invites 子命令注册

def patch(path, pairs):
    s = open(path, encoding='utf-8').read()
    for old, new in pairs:
        if old not in s:
            print("MISS", path, old.splitlines()[0][:70]); raise SystemExit(1)
        s = s.replace(old, new, 1)
    open(path, 'w', encoding='utf-8').write(s)
    print("patched", path)

patch('apps/cli/src/chat/friends.rs', [
# 文件头注释
("""//! chat friends 子域：list / add / update / remove（契约 §12.1，IM-T43 增分组）。
//!
//! add 幂等（upsert，R3）：重复添加更新昵称/地址不报错，--json created 可判；
//! remove 幂等：不在簿 removed=false 仍退出 0（对齐 GUI bool 返回语义）；
//! update 走 friend_update.rs 子域；--group 过滤与分组展示未分组恒置底。""",
"""//! chat friends 子域：list / add / update / remove / invites（契约 §12.1/§12.4）。
//!
//! add = 发好友邀请（邀请制：对方同意前不建好友，delivered 可判送达/挂起）；
//! remove 幂等：不在簿 removed=false 仍退出 0（对齐 GUI bool 返回语义）；
//! update 走 friend_update.rs 子域；invites 走 friend_invites.rs 子域；
//! --group 过滤与分组展示未分组恒置底。"""
),
# 枚举注册
("""#[derive(Subcommand)]
pub enum FriendsCommand {
    /// 列出全部好友（--group 过滤；默认按分组展示，未分组置底）
    List(ListArgs),
    /// 添加好友（幂等 upsert；已在簿则为更新并标记 created=false）
    Add(AddArgs),
    /// 更新好友（分组/昵称/备注补丁，至少提供一项；addrs 不可经此修改）
    Update(UpdateArgs),
    /// 移除好友（幂等；不在簿返回 removed=false）
    Remove(RemoveArgs),
}""",
"""#[derive(Subcommand)]
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
    Invites(super::friend_invites::InvitesCommand),
}"""
),
# AddArgs 删 --group
("""    /// 备注（可选）
    #[arg(long)]
    note: Option<String>,
    /// 分组（可选；trim 后 ≤32 字符，空串 = 不分组）
    #[arg(long)]
    group: Option<String>,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
    /// 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Args)]
pub struct RemoveArgs {""",
"""    /// 备注（可选）
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
pub struct RemoveArgs {"""
),
# 报告结构替换
("""/// add 幂等报告：created=false 表示该 peer 已在簿（本次为更新）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FriendAddReport {
    created: bool,
    friend: ChatFriend,
}

#[derive(Serialize)]""",
"""#[derive(Serialize)]"""
),
# run 分发
("""    match command {
        FriendsCommand::List(args) => list(args).await,
        FriendsCommand::Add(args) => add(args).await,
        FriendsCommand::Update(args) => super::friend_update::run(args).await,
        FriendsCommand::Remove(args) => remove(args).await,
    }""",
"""    match command {
        FriendsCommand::List(args) => list(args).await,
        FriendsCommand::Add(args) => add(args).await,
        FriendsCommand::Update(args) => super::friend_update::run(args).await,
        FriendsCommand::Remove(args) => remove(args).await,
        FriendsCommand::Invites(command) => super::friend_invites::run(command).await,
    }"""
),
# add 函数体
("""async fn add(args: AddArgs) -> CliResult<()> {
    let ctx = context::open(&args.data_dir).await?;
    let existed = ctx
        .chat
        .friends_list()
        .map_err(runtime_err)?
        .iter()
        .any(|f| f.peer_id == args.peer_id);
    let mut friend = ctx
        .chat
        .friend_add(
            &args.peer_id,
            &args.nickname,
            args.addr.clone(),
            args.note.clone(),
        )
        .map_err(runtime_err)?;
    if args.group.is_some() {
        // 分组经 friend_update 补丁生效（friend_add 签名保持既有调用面）；
        // add 成功而组名校验被拒时好友已在簿，重跑 add（upsert）+ 合法组名即恢复。
        friend = ctx
            .chat
            .friend_update(
                &args.peer_id,
                &p2p_chat::FriendPatch {
                    group: args.group.clone(),
                    ..Default::default()
                },
            )
            .map_err(runtime_err)?;
    }
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
}""",
"""async fn add(args: AddArgs) -> CliResult<()> {
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
}"""
),
# add 报告测试替换
("""    #[test]
    fn add_report_json_shape_is_judgeable() {
        let report = FriendAddReport {
            created: false,
            friend: fixture("b", Some("同事")),
        };
        let v = serde_json::to_value(&report).unwrap();
        assert_eq!(v["created"], serde_json::json!(false));
        assert_eq!(v["friend"]["peerId"], serde_json::json!("p"));
        assert_eq!(v["friend"]["group"], serde_json::json!("同事"));
    }""",
"""    #[test]
    fn invite_report_json_shape_is_judgeable() {
        let report = p2p_chat::InviteReport {
            delivered: true,
            invite: fixture("b", Some("同事")),
        };
        let v = serde_json::to_value(&report).unwrap();
        assert_eq!(v["delivered"], serde_json::json!(true));
        assert_eq!(v["invite"]["peerId"], serde_json::json!("p"));
        assert_eq!(v["invite"]["group"], serde_json::json!("同事"));
    }"""
),
])

# mod.rs 注册 invites 子域
patch('apps/cli/src/chat/mod.rs', [(
"""pub mod context;
mod friend_update;
mod friends;""",
"""pub mod context;
mod friend_invites;
mod friend_update;
mod friends;"""
)])
