//! group invites 子域测试：clap 解析（四入口/位置参数/默认值）、子树注册、
//! JSON 输出形状与离线可观察文本。门面语义归 p2p-chat 测试，此处不重复。

use clap::{CommandFactory, Parser};

use super::{delivery_note, invite_line, InvitesCommand};
use crate::cli::{Cli, Command};
use crate::group::GroupCommand;

fn parse(args: &[&str]) -> InvitesCommand {
    let mut full: Vec<&str> = vec!["p2pctl"];
    full.extend_from_slice(args);
    match Cli::try_parse_from(full).expect("应解析成功").command {
        Command::Group {
            command: GroupCommand::Invites { command },
        } => command,
        _ => panic!("期望 group invites 子命令"),
    }
}

fn fixture(direction: p2p_chat::GroupInviteDirection) -> p2p_chat::GroupInvite {
    p2p_chat::GroupInvite {
        id: "i1".into(),
        group_id: "g1".into(),
        group_name: "群".into(),
        owner: "o".into(),
        inviter: "o".into(),
        invitee: "e".into(),
        note: None,
        direction,
        state: p2p_chat::GroupInviteState::Pending,
        ts_ms: 1,
        delivered: false,
    }
}

#[test]
fn send_parses_targets_note_and_json() {
    let cmd = parse(&[
        "group", "invites", "send", "--group", "g1", "--peer", "p1", "--note", "hi", "--json",
    ]);
    let InvitesCommand::Send(args) = cmd else {
        panic!("期望 send");
    };
    assert_eq!(args.group, "g1");
    assert_eq!(args.peer, "p1");
    assert_eq!(args.note.as_deref(), Some("hi"), "note 透传");
    assert!(args.json);
    assert_eq!(args.nickname, "", "昵称缺省空串=PeerId 缩略回退");
}

#[test]
fn accept_takes_positional_invite_id_and_reject_reason_optional() {
    let InvitesCommand::Accept(args) = parse(&["group", "invites", "accept", "i-9"]) else {
        panic!("期望 accept");
    };
    assert_eq!(args.invite_id, "i-9", "位置参数 inviteId 对齐契约");
    assert!(!args.json);

    let InvitesCommand::Reject(args) =
        parse(&["group", "invites", "reject", "i-9", "--reason", "忙"])
    else {
        panic!("期望 reject");
    };
    assert_eq!(args.invite_id, "i-9");
    assert_eq!(args.reason.as_deref(), Some("忙"));

    let InvitesCommand::Reject(args) = parse(&["group", "invites", "reject", "i-9"]) else {
        panic!("期望 reject");
    };
    assert!(args.reason.is_none(), "reason 可省略");
}

#[test]
fn invites_subtree_registers_all_four_verbs() {
    let cmd = Cli::command();
    let group = cmd
        .find_subcommand("group")
        .expect("group 域已注册")
        .find_subcommand("invites")
        .expect("invites 子树已注册");
    for verb in ["send", "list", "accept", "reject"] {
        assert!(group.find_subcommand(verb).is_some(), "缺 {verb} 入口");
    }
}

#[test]
fn send_report_json_carries_top_level_delivered_signal() {
    let report = p2p_chat::GroupInviteReport {
        invite: fixture(p2p_chat::GroupInviteDirection::Out),
        delivered: false,
    };
    let v = serde_json::to_value(&report).expect("serialize");
    assert_eq!(v["delivered"], serde_json::json!(false), "离线信号顶层可判");
    assert_eq!(v["invite"]["groupId"], "g1", "条目 camelCase 与契约一致");
    assert_eq!(v["invite"]["state"], "pending");
}

#[test]
fn text_mode_marks_direction_state_and_offline() {
    assert!(delivery_note(false).contains("未送达"), "离线必须可观察");
    assert!(delivery_note(true).contains("已送达"));
    let line = invite_line(&fixture(p2p_chat::GroupInviteDirection::In));
    assert!(line.contains("in"), "方向可判: {line}");
    assert!(line.contains("状态=pending"), "状态可判: {line}");
    assert!(line.contains("未送达"), "离线可观察: {line}");
}
