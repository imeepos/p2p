//! config 单测（workspace 解析与 legacy 兜底）。
use std::collections::BTreeMap;

use super::{AgentConfig, ConfigError, WorkspaceDef, DEFAULT_WORKSPACE_ID};
use crate::config::load_file;

#[test]
fn defaults_match_design() {
    let cfg = AgentConfig::default();
    assert_eq!(cfg.protocol_id, "/dsh-acp/1");
    assert_eq!(cfg.command, vec!["pnpm", "dsh", "--profile", "acp"]);
    assert_eq!(cfg.max_connections, 8);
    assert_eq!(cfg.grace_secs, 10);
    cfg.validate().expect("defaults must validate");
}

#[test]
fn partial_file_fills_defaults() {
    let dir = std::env::temp_dir().join(format!("acp-agent-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tmp dir");
    let path = dir.join("agent.json");
    let body = serde_json::json!({ "data_dir": "/tmp/x" }).to_string();
    std::fs::write(&path, body).expect("write");
    let cfg = load_file(path.to_str().expect("utf8")).expect("parse");
    assert_eq!(cfg.data_dir, "/tmp/x");
    assert_eq!(cfg.protocol_id, "/dsh-acp/1");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mcp_definitions_file_loads_and_wins() {
    let dir = std::env::temp_dir().join(format!("acp-mcp-load-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tmp dir");
    let path = dir.join("mcp.json");
    std::fs::write(&path, "{\"fs\":{\"command\":\"node\"}}").expect("write");
    let mut cfg = AgentConfig {
        mcp_definitions_path: Some(path.to_string_lossy().into_owned()),
        mcp_definitions: BTreeMap::from([("inline".to_owned(), serde_json::json!({}))]),
        ..AgentConfig::default()
    };
    cfg.load_mcp_definitions().expect("load");
    assert!(cfg.mcp_definitions.contains_key("fs"));
    assert!(!cfg.mcp_definitions.contains_key("inline"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mcp_definitions_corrupt_file_is_explicit_error() {
    let dir = std::env::temp_dir().join(format!("acp-mcp-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tmp dir");
    let path = dir.join("mcp.json");
    std::fs::write(&path, "not json").expect("write");
    let mut cfg = AgentConfig {
        mcp_definitions_path: Some(path.to_string_lossy().into_owned()),
        ..AgentConfig::default()
    };
    let err = cfg.load_mcp_definitions().expect_err("must fail");
    assert!(matches!(err, ConfigError::McpFileJson { .. }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mcp_definitions_array_shape_is_rejected() {
    let dir = std::env::temp_dir().join(format!("acp-mcp-shape-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tmp dir");
    let path = dir.join("mcp.json");
    std::fs::write(&path, "[]").expect("write");
    let mut cfg = AgentConfig {
        mcp_definitions_path: Some(path.to_string_lossy().into_owned()),
        ..AgentConfig::default()
    };
    let err = cfg.load_mcp_definitions().expect_err("must fail");
    assert!(matches!(err, ConfigError::McpFileShape { .. }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn validate_rejects_empty_command() {
    let mut cfg = AgentConfig::default();
    cfg.command.clear();
    assert!(cfg.validate().is_err());
}

#[test]
fn workspace_resolves_explicit_rows() {
    let cfg = AgentConfig {
        workspaces: vec![WorkspaceDef {
            id: "ws1".to_owned(),
            name: "p2p".to_owned(),
            dir: "/tmp/ws1".to_owned(),
        }],
        ..AgentConfig::default()
    };
    let hit = cfg.workspace(Some("ws1")).expect("ws1");
    assert_eq!(hit.dir, "/tmp/ws1");
    assert!(cfg.workspace(Some("ws2")).is_none());
    assert!(
        cfg.workspace(None).is_none(),
        "无 legacy 配置时无默认工作区"
    );
}

#[test]
fn workspace_none_falls_back_to_legacy_dir() {
    let cfg = AgentConfig {
        workspace_dir: Some("/tmp/legacy".to_owned()),
        ..AgentConfig::default()
    };
    let hit = cfg.workspace(None).expect("legacy default");
    assert_eq!(hit.id, DEFAULT_WORKSPACE_ID);
    assert_eq!(hit.dir, "/tmp/legacy");
}

#[test]
fn workspace_rows_merge_legacy_without_dup() {
    let mut cfg = AgentConfig {
        workspace_dir: Some("/tmp/legacy".to_owned()),
        ..AgentConfig::default()
    };
    assert_eq!(cfg.workspace_rows().len(), 1);
    cfg.workspaces = vec![WorkspaceDef {
        id: DEFAULT_WORKSPACE_ID.to_owned(),
        name: "main".to_owned(),
        dir: "/tmp/legacy".to_owned(),
    }];
    assert_eq!(cfg.workspace_rows().len(), 1, "显式 default 表项不重复兜底");
}
