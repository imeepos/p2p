//! acp share 域测试：参数解析、只存哈希、链接要素、级联撤销与脱敏列表。

use std::fs;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use acp_common::share::ShareLedger;
use clap::Parser;

use super::share::{ShareArgs, ShareCommand};
use super::store;
use crate::cli::{Cli, Command};

fn temp_dir(tag: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("时钟正常")
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("p2pctl-acp-share-{tag}-{}-{nanos}", process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir.to_string_lossy().into_owned()
}

fn parse(args: &[&str]) -> Command {
    let mut full: Vec<&str> = vec!["p2pctl"];
    full.extend_from_slice(args);
    Cli::try_parse_from(full).expect("应解析成功").command
}

fn expect_create(args: &[&str]) -> super::share::ShareCreateArgs {
    match parse(args) {
        Command::Acp {
            command:
                super::AcpCommand::Share(ShareArgs {
                    command: ShareCommand::Create(create),
                }),
        } => create,
        _ => panic!("应解析为 acp share create"),
    }
}

/// 种入 agent 节点身份（链接 peer 要素来源；load-only 语义的前提）。
fn seed_identity(data_dir: &str) -> String {
    let keypair = p2p_identity::Keypair::generate();
    let seed = acp_common::AcpPaths::new(data_dir)
        .root
        .join("identity")
        .join("key.seed");
    p2p_identity::save_seed(&seed, &keypair).expect("seed identity");
    keypair.peer_id().to_string()
}

#[test]
fn create_args_parse_with_defaults() {
    let args = expect_create(&[
        "acp",
        "share",
        "create",
        "--ttl-secs",
        "3600",
        "--data-dir",
        "d",
    ]);
    assert_eq!(args.ttl_secs, 3600);
    assert_eq!(args.max_activations, 1, "默认一次性");
    assert!(matches!(args.scope, super::ScopeArg::Sandbox));
    assert!(matches!(args.ask_route, super::AskRouteArg::RemoteGui));
    assert_eq!(args.data_dir, "d");
}

#[test]
fn share_tree_leaves_exist_for_help_path() {
    // 验收口径：acp share --help 之下应有 create/list/revoke 三个叶子（逐一实测解析）。
    expect_create(&["acp", "share", "create", "--ttl-secs", "1"]);
    match parse(&["acp", "share", "list"]) {
        Command::Acp {
            command:
                super::AcpCommand::Share(ShareArgs {
                    command: ShareCommand::List(_),
                }),
        } => {}
        _ => panic!("应解析为 acp share list"),
    }
    match parse(&["acp", "share", "revoke", "some-id"]) {
        Command::Acp {
            command:
                super::AcpCommand::Share(ShareArgs {
                    command: ShareCommand::Revoke(_),
                }),
        } => {}
        _ => panic!("应解析为 acp share revoke"),
    }
}

#[test]
fn create_writes_ledger_with_hash_and_link_without_plaintext() {
    let data_dir = temp_dir("create");
    let peer = seed_identity(&data_dir);
    let args = super::share::ShareCreateArgs {
        scope: super::ScopeArg::Sandbox,
        ttl_secs: 3_600,
        max_activations: 1,
        allow_mcp: vec!["fs".to_owned()],
        ask_route: super::AskRouteArg::RemoteGui,
        note: Some("nb".to_owned()),
        addrs: vec!["/ip4/127.0.0.1/udp/4001/quic-v1".to_owned()],
        data_dir: data_dir.clone(),
    };
    let report = super::share::create_share(&args).expect("create");
    assert_eq!(report.peer, peer);
    assert_eq!(report.token.len(), 32);
    assert_eq!(
        report.link,
        format!(
            "dsh-acp-share://v1?peer={peer}&addr=/ip4/127.0.0.1/udp/4001/quic-v1&token={}&exp={}&sid={}",
            report.token,
            report.expires_at_unix,
            report.share_id,
        ),
    );
    let path = store::shares_path(&data_dir);
    let raw = fs::read_to_string(&path).expect("ledger");
    assert!(!raw.contains(&report.token), "token 原文不得落盘");
    assert!(
        raw.contains(&acp_common::token_sha256(&report.token)),
        "台账只存哈希"
    );
    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn create_requires_agent_identity_and_rejects_zero_ttl() {
    let data_dir = temp_dir("noid");
    let args = super::share::ShareCreateArgs {
        scope: super::ScopeArg::Sandbox,
        ttl_secs: 0,
        max_activations: 1,
        allow_mcp: Vec::new(),
        ask_route: super::AskRouteArg::RemoteGui,
        note: None,
        addrs: Vec::new(),
        data_dir: data_dir.clone(),
    };
    let err = super::share::create_share(&args).expect_err("ttl=0 必须拒绝");
    assert!(err.to_string().contains("--ttl-secs"));
    let args = super::share::ShareCreateArgs {
        ttl_secs: 60,
        ..args
    };
    let err = super::share::create_share(&args).expect_err("无身份必须拒绝");
    assert!(
        err.to_string().contains("acp-agent"),
        "报错应指向启动 agent: {err}"
    );
    let _ = fs::remove_dir_all(&data_dir);
}

#[test]
fn create_rejects_workspace_scope_fail_closed() {
    let data_dir = temp_dir("ws");
    seed_identity(&data_dir);
    let args = super::share::ShareCreateArgs {
        scope: super::ScopeArg::Workspace,
        ttl_secs: 60,
        max_activations: 1,
        allow_mcp: Vec::new(),
        ask_route: super::AskRouteArg::RemoteGui,
        note: None,
        addrs: Vec::new(),
        data_dir,
    };
    let err = super::share::create_share(&args).expect_err("workspace 必须拒绝");
    assert!(err.to_string().contains("workspace"));
}

#[test]
fn revoke_redacts_and_cascades_share_sourced_policy() {
    let data_dir = temp_dir("revoke");
    seed_identity(&data_dir);
    let create_args = super::share::ShareCreateArgs {
        scope: super::ScopeArg::Sandbox,
        ttl_secs: 3_600,
        max_activations: 1,
        allow_mcp: Vec::new(),
        ask_route: super::AskRouteArg::RemoteGui,
        note: None,
        addrs: Vec::new(),
        data_dir: data_dir.clone(),
    };
    let report = super::share::create_share(&create_args).expect("create");

    // 预置该 peer 的 share 来源策略条目（模拟兑换已发生）。
    let policy_path = store::policy_path(&data_dir);
    let mut table = store::load_or_empty(&policy_path).expect("policy");
    table.grant(
        "PEER_A",
        acp_common::PeerPolicy {
            scope: acp_common::policy::Scope::Sandbox,
            allow_mcp: Vec::new(),
            ask_route: acp_common::policy::AskRoute::RemoteGui,
            note: String::new(),
            granted_at: "2026-01-01T00:00:00Z".to_owned(),
            fingerprint: format!("share:{}", report.share_id),
            workspace: None,
        },
    );
    store::save(&policy_path, &table).expect("save policy");

    // 模拟激活已发生：台账绑定 PEER_A（设计 §3：级联只对已绑定 peer 生效）。
    let shares_path = store::shares_path(&data_dir);
    let mut ledger = store::load_shares_or_empty(&shares_path).expect("ledger");
    ledger.get_mut(&report.share_id).expect("entry").bound_peer = Some("PEER_A".to_owned());
    store::save_shares(&shares_path, &ledger).expect("save ledger");

    let list = super::share::list_entries(&data_dir).expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].status, "bound", "已绑定未用尽显示 bound");

    let revoke_args = super::share::ShareRevokeArgs {
        share_id: report.share_id.clone(),
        json: true,
        data_dir: data_dir.clone(),
    };
    let revoked = super::share::revoke_share(&revoke_args).expect("revoke");
    assert!(revoked.policy_removed, "share 来源条目应级联删除");
    let table = store::load_or_empty(&policy_path).expect("policy");
    assert!(table.lookup("PEER_A").is_none());
    let ledger = ShareLedger::load(&store::shares_path(&data_dir)).expect("ledger");
    assert!(ledger.get(&report.share_id).expect("entry").revoked);

    let err = super::share::revoke_share(&super::share::ShareRevokeArgs {
        share_id: "nope".to_owned(),
        json: true,
        data_dir: data_dir.clone(),
    })
    .expect_err("未知 id 必须报错");
    assert!(err.to_string().contains("无此 share_id"));
    let _ = fs::remove_dir_all(&data_dir);
}