//! acp 域测试：clap 参数解析/枚举校验/PeerId 校验/空表与损坏表/临时目录端到端。

use std::fs;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use acp_common::policy::Scope;
use clap::Parser;

use super::{
    allow_policy, deny_policy, list_entries, validate_peer_id, AcpCommand, AllowArgs, AskRouteArg,
    DenyArgs, ListArgs, ScopeArg,
};
use crate::cli::{Cli, Command};

/// doc 例采样的真实形态 base58(32B) PeerId。
const PEER: &str = "HCjw5d6mzG5Z9iGTebhRSHBZKjA1WuunTXkZN9gzmfWj";

fn temp_dir(tag: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("时钟正常")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("p2pctl-acp-{tag}-{}-{nanos}", process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir.to_string_lossy().into_owned()
}

fn parse(args: &[&str]) -> Command {
    let mut full: Vec<&str> = vec!["p2pctl"];
    full.extend_from_slice(args);
    Cli::try_parse_from(full).expect("应解析成功").command
}

fn allow_args(data_dir: &str) -> AllowArgs {
    AllowArgs {
        peer_id: PEER.to_owned(),
        scope: ScopeArg::Sandbox,
        allow_mcp: vec!["fs".to_owned()],
        ask_route: AskRouteArg::RemoteGui,
        note: Some("nb".to_owned()),
        fingerprint: Some("ff00".to_owned()),
        json: false,
        data_dir: data_dir.to_owned(),
    }
}

#[test]
fn allow_defaults_scope_sandbox_and_route_remote_gui() {
    match parse(&["acp", "allow", PEER]) {
        Command::Acp {
            command: AcpCommand::Allow(args),
        } => {
            assert_eq!(args.peer_id, PEER);
            assert!(matches!(args.scope, ScopeArg::Sandbox));
            assert!(matches!(args.ask_route, AskRouteArg::RemoteGui));
            assert!(args.allow_mcp.is_empty());
            assert_eq!(args.note, None);
            assert_eq!(args.fingerprint, None);
        }
        _ => panic!("应解析为 acp allow"),
    }
}

#[test]
fn allow_accepts_multi_mcp_and_full_options() {
    let parsed = parse(&[
        "acp",
        "allow",
        PEER,
        "--scope",
        "workspace",
        "--allow-mcp",
        "fs",
        "--allow-mcp",
        "web",
        "--ask-route",
        "owner_local",
        "--note",
        "n1",
        "--fingerprint",
        "ab12",
    ]);
    match parsed {
        Command::Acp {
            command: AcpCommand::Allow(args),
        } => {
            assert!(matches!(args.scope, ScopeArg::Workspace));
            assert_eq!(args.allow_mcp, vec!["fs", "web"]);
            assert!(matches!(args.ask_route, AskRouteArg::OwnerLocal));
            assert_eq!(args.note.as_deref(), Some("n1"));
            assert_eq!(args.fingerprint.as_deref(), Some("ab12"));
        }
        _ => panic!("应解析为 acp allow"),
    }
}

#[test]
fn illegal_enum_value_is_usage_error() {
    assert!(Cli::try_parse_from(["p2pctl", "acp", "allow", PEER, "--scope", "root"]).is_err());
    assert!(Cli::try_parse_from([
        "p2pctl",
        "acp",
        "allow",
        PEER,
        "--ask-route",
        "carrier_pigeon"
    ])
    .is_err());
}

#[test]
fn peer_id_validation_aligns_project_rule() {
    assert!(validate_peer_id(PEER).is_ok());
    assert!(validate_peer_id("!!!not-base58!!!").is_err());
    let short = bs58::encode([0u8; 31]).into_string();
    assert!(validate_peer_id(&short).is_err());
}

#[test]
fn mcp_names_reject_blank_and_dedupe() {
    let args = AllowArgs {
        allow_mcp: vec!["fs".to_owned(), " fs ".to_owned(), String::new()],
        ..allow_args("/tmp/unused")
    };
    assert!(super::dedupe_mcp_names(&args.allow_mcp).is_err());
    let duped = vec!["fs".to_owned(), " fs ".to_owned(), "web".to_owned()];
    assert_eq!(
        super::dedupe_mcp_names(&duped).expect("去重成功"),
        vec!["fs", "web"]
    );
}

#[test]
fn allow_list_deny_roundtrip_on_injected_dir() {
    let dir = temp_dir("e2e");
    let mut allow = allow_args(&dir);

    let first = allow_policy(&allow).expect("首授成功");
    assert!(first.created);

    let entries = list_entries(&ListArgs {
        json: false,
        data_dir: dir.clone(),
    })
    .expect("list 成功");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].peer_id, PEER);
    assert_eq!(entries[0].allow_mcp, vec!["fs"]);
    assert_eq!(entries[0].fingerprint, "ff00");
    assert!(matches!(entries[0].scope, Scope::Sandbox));
    assert!(super::render::render_list(&entries).contains(PEER));

    allow.scope = ScopeArg::Workspace;
    allow.allow_mcp.clear();
    let again = allow_policy(&allow).expect("二次授权成功");
    assert!(!again.created, "同 peer 二次授权应为 upsert 更新");
    let entries = list_entries(&ListArgs {
        json: false,
        data_dir: dir.clone(),
    })
    .expect("list 成功");
    assert!(matches!(entries[0].scope, Scope::Workspace));
    assert!(entries[0].allow_mcp.is_empty(), "upsert 应整体替换白名单");

    let deny = deny_policy(&DenyArgs {
        peer_id: PEER.to_owned(),
        json: false,
        data_dir: dir.clone(),
    })
    .expect("deny 成功");
    assert!(deny.removed);
    assert!(list_entries(&ListArgs {
        json: false,
        data_dir: dir.clone()
    })
    .expect("list 成功")
    .is_empty());

    let missing = deny_policy(&DenyArgs {
        peer_id: PEER.to_owned(),
        json: false,
        data_dir: dir,
    });
    assert!(missing.is_err(), "deny 不存在条目必须明确报错");
}

#[test]
fn missing_file_lists_as_empty_table() {
    let dir = temp_dir("empty");
    let entries = list_entries(&ListArgs {
        json: false,
        data_dir: dir.clone(),
    })
    .expect("缺失文件视为空表");
    assert!(entries.is_empty());
    assert!(super::render::render_list(&entries).starts_with("策略表为空"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn corrupted_table_fails_loudly() {
    let dir = temp_dir("corrupt");
    fs::create_dir_all(&dir).expect("建临时目录");
    fs::write(super::store::policy_path(&dir), "{ not json").expect("写损坏样本");
    let read = list_entries(&ListArgs {
        json: false,
        data_dir: dir.clone(),
    })
    .expect_err("损坏表必须显式报错");
    assert!(read.to_string().contains("策略表读取失败"));
    assert!(
        allow_policy(&allow_args(&dir)).is_err(),
        "损坏表禁止静默覆盖"
    );
    let _ = fs::remove_dir_all(&dir);
}
