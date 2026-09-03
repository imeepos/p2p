//! T23 宿主级接线测试：guarded Host 的执法/审计/输出门禁/工具注册。
//! 独立模块文件（_tests 约定），只覆盖本批新增行为，不动 T21 宿主测试。

use super::*;
use crate::audit::AuditSink;
use crate::enforce::Enforcement;
use crate::jail::PathJail;
use crate::tools;
use repair_enforce::{scope::Scope, whitelist::ShellWhitelist};
use std::path::PathBuf;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::watch;

fn fixture_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("rh-host-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("a.txt"), "hello").unwrap();
    root
}

async fn run_guarded(input: &str, jail: PathJail) -> (String, AuditSink) {
    let registry = tools::read_only_registry(jail);
    let audit = AuditSink::default();
    let host = Host::guarded(
        registry,
        Enforcement::new(Scope::Diag, ShellWhitelist::empty()),
        audit.clone(),
    );
    let (mut client, server) = duplex(1 << 20);
    let (reader, writer) = tokio::io::split(server);
    let (_tx, rx) = watch::channel(false);
    let task = tokio::spawn(host.serve(BufReader::new(reader), writer, rx));
    client.write_all(input.as_bytes()).await.unwrap();
    client.shutdown().await.unwrap();
    let mut output = String::new();
    BufReader::new(client)
        .read_to_string(&mut output)
        .await
        .unwrap();
    let _ = task.await;
    (output, audit)
}

#[tokio::test]
async fn guarded_tools_list_contains_four_readonly() {
    let jail = PathJail::from_roots(vec![fixture_root("list")]).unwrap();
    let (output, _) = run_guarded(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n",
        jail,
    )
    .await;
    for name in ["sys_snapshot", "fs_read", "fs_list", "fs_search"] {
        assert!(output.contains(name), "missing {name} in {output}");
    }
}

#[tokio::test]
async fn guarded_fs_read_happy_path() {
    let jail = PathJail::from_roots(vec![fixture_root("read")]).unwrap();
    let (output, audit) = run_guarded(
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"fs_read\",\"arguments\":{\"path\":\"a.txt\"}}}\n",
        jail,
    )
    .await;
    assert!(output.contains("\"text\":\"hello\""), "{}", output);
    assert!(output.contains("\"isError\":false"), "{}", output);
    let events = audit.snapshot();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].tool, "fs_read");
    assert_eq!(events[0].risk, "read");
    assert_eq!(events[0].outcome, "ok");
}

#[tokio::test]
async fn guarded_write_tier_denied_with_reason() {
    let jail = PathJail::from_roots(vec![fixture_root("deny")]).unwrap();
    let (output, audit) = run_guarded(
        "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"fs_write\",\"arguments\":{\"path\":\"a.txt\"}}}\n",
        jail,
    )
    .await;
    assert!(output.contains("\"isError\":true"), "{}", output);
    assert!(output.contains("denied"), "{}", output);
    let events = audit.snapshot();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].outcome, "denied");
    assert_eq!(events[0].risk, "write");
}

#[tokio::test]
async fn guarded_jail_escape_returns_tool_error_with_reason() {
    let root = fixture_root("esc");
    let outside = root.parent().unwrap().join("outside.txt");
    std::fs::write(&outside, "x").unwrap();
    let jail = PathJail::from_roots(vec![root]).unwrap();
    let (output, audit) = run_guarded(
        "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"fs_read\",\"arguments\":{\"path\":\"../outside.txt\"}}}\n",
        jail,
    )
    .await;
    assert!(output.contains("\"isError\":true"), "{}", output);
    assert!(output.contains(".."), "{}", output);
    assert_eq!(audit.snapshot()[0].outcome, "error");
}

#[tokio::test]
async fn guarded_fs_read_truncation_flagged_through_host() {
    let root = fixture_root("trunc");
    std::fs::write(
        root.join("big.txt"),
        "a".repeat(crate::cap::MAX_OUTPUT_BYTES + 4096),
    )
    .unwrap();
    let jail = PathJail::from_roots(vec![root]).unwrap();
    let (output, audit) = run_guarded(
        "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"tools/call\",\"params\":{\"name\":\"fs_read\",\"arguments\":{\"path\":\"big.txt\"}}}\n",
        jail,
    )
    .await;
    assert!(output.contains("\"truncated\":true"), "{}", output);
    let events = audit.snapshot();
    assert_eq!(events[0].tool, "fs_read");
    assert!(events[0].result_summary.len() <= 120);
}

#[tokio::test]
async fn guarded_sys_snapshot_recorded_in_audit() {
    let jail = PathJail::from_roots(vec![fixture_root("snap")]).unwrap();
    let (output, audit) = run_guarded(
        "{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"tools/call\",\"params\":{\"name\":\"sys_snapshot\",\"arguments\":{}}}\n",
        jail,
    )
    .await;
    assert!(
        output.contains("os=") && output.contains("\"isError\":false"),
        "{}",
        output
    );
    let events = audit.snapshot();
    assert_eq!(events[0].tool, "sys_snapshot");
    assert_eq!(events[0].risk, "read");
    assert_eq!(events[0].outcome, "ok");
    assert!(events[0].at_unix_ms > 1_700_000_000_000);
}
