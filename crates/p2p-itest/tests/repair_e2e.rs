//! T27 桥⇄助手全链 E2E itest（remote-support-plan.md §4 T27，依赖 T20/T26）。
//! 断言用例集中本文件；真实双节点/桥 pump/票据/夹具在 common 模块。
//! 时序纪律（coordination.md 规则 8）：全部等待显式 timeout，无长 sleep。

mod common;
use common::*;
use p2p::{BoxedStream, ProtocolId};
use p2p_protocol::{read_frame, write_frame};
use repair_bridge::PROTOCOL_ID;
use repair_enforce::whitelist::{ArgPat, ShellRule, ShellWhitelist};
use repair_enforce::{ApprovalVerdict, Approver};
use repair_helper::ticket::{SCOPE_DIAG, SCOPE_FIX};
use repair_helper::tools::approval::QueueApprover;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;

/// 全链断言链：initialize → tools/list 六工具 → sys_snapshot → fs_read →
/// 未知方法 -32601 → session_report 含调用记录。
#[tokio::test]
async fn full_chain_serves_mcp_over_real_transport() {
    let (helper, client, mut s, root) = rig("chain", "t-chain").await;
    let init = rpc(&mut s, 1, "initialize", INIT_V18).await;
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18", "{init}");
    let list = rpc(&mut s, 2, "tools/list", "{}").await;
    let tools = list["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names.len(), 6, "{names:?}");
    let want = [
        "sys_snapshot",
        "fs_read",
        "fs_list",
        "fs_search",
        "shell_exec",
        "session_report",
    ];
    for w in want {
        assert!(names.contains(&w), "missing {w}: {names:?}");
    }
    let snap = call_tool(&mut s, 3, "sys_snapshot", "{}").await;
    let text = snap["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("os=") && text.contains("arch="), "{text}");
    let fs = call_tool(&mut s, 4, "fs_read", "{\"path\":\"a.txt\"}").await;
    assert_eq!(fs["result"]["content"][0]["text"], "hello", "{fs}");
    let unk = rpc(&mut s, 5, "tools/nope", "{}").await;
    assert_eq!(unk["error"]["code"], -32601, "{unk}");
    let report = call_tool(&mut s, 6, "session_report", "{}").await;
    let body = report["result"]["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(body).unwrap();
    assert_eq!(parsed["ticketId"], "t-chain", "{body}");
    assert!(parsed["count"].as_u64().unwrap() >= 2, "{body}");
    assert!(
        body.contains("sys_snapshot") && body.contains("fs_read"),
        "{body}"
    );
    drain(s).await;
    teardown(&helper, &client, root);
}

/// diag scope 下 write 类调用秒级拒绝带原因（P0b 无审批通道，不真等 60s）。
#[tokio::test]
async fn diag_write_call_denied_with_reason() {
    let (helper, client, mut s, root) = rig("deny", "t-deny").await;
    let resp = call_tool(&mut s, 1, "fs_write", "{\"path\":\"a.txt\"}").await;
    assert_eq!(resp["result"]["isError"], true, "{resp}");
    let text = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("denied"), "{text}");
    assert!(text.contains("diag"), "{text}");
    let report = call_tool(&mut s, 2, "session_report", "{}").await;
    let body = report["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        body.contains("fs_write") && body.contains("denied"),
        "{body}"
    );
    drain(s).await;
    teardown(&helper, &client, root);
}

/// 断线语义（§3.7）：流断后受理结束；同 ticket 二次受理拒绝；新票据可再受理。
#[tokio::test]
async fn stream_break_ends_acceptance_same_ticket_rejected() {
    let root = fixture_root("disc");
    let helper = helper_node(
        "disc",
        root.clone(),
        ShellWhitelist::empty(),
        noop_approver(),
    )
    .await;
    let helper_peer = helper.local_peer_id();
    let client = client_node("disc", &helper, helper_peer).await;
    let ticket = fresh_ticket("t-once", SCOPE_DIAG, helper_peer, client.local_peer_id());
    let mut s1 = open_session(&client, helper_peer, &ticket).await;
    let init = rpc(&mut s1, 1, "initialize", INIT_V26).await;
    assert_eq!(init["result"]["protocolVersion"], "2025-03-26", "{init}");
    drain(s1).await;
    // 同 ticket 二次受理：断旧连重拨新连接（yamux 空闲二次开流有缺陷），读侧拒
    client.disconnect(&helper_peer);
    client.connect(helper_peer).await.unwrap();
    let protocol = ProtocolId::new(PROTOCOL_ID).unwrap();
    let mut stream2: BoxedStream = client.new_stream(helper_peer, protocol).await.unwrap();
    write_frame(&mut stream2, ticket.as_bytes()).await.unwrap();
    stream2.flush().await.unwrap();
    tokio::time::timeout(STEP, read_frame(&mut stream2))
        .await
        .expect("second accept timed out")
        .expect_err("same ticket second accept must be rejected");
    drop(stream2);
    // 新票据：helper 仍可受理（断线未拖垮端点）；同样换新连接
    client.disconnect(&helper_peer);
    client.connect(helper_peer).await.unwrap();
    let ticket2 = fresh_ticket("t-fresh", SCOPE_DIAG, helper_peer, client.local_peer_id());
    let mut s3 = open_session(&client, helper_peer, &ticket2).await;
    let init2 = rpc(&mut s3, 1, "initialize", INIT_V18).await;
    assert_eq!(init2["result"]["protocolVersion"], "2025-06-18", "{init2}");
    drain(s3).await;
    teardown(&helper, &client, root);
}

/// shell_exec 白名单命中路径（§3.4/§3.5，执行面 T23b 已合入）：
/// fix scope + 白名单闭集内命令 + 预批通道 → 执行返回输出；闭集外拒绝带原因。
#[tokio::test]
async fn shell_exec_whitelist_hit_and_miss() {
    let mut whitelist = ShellWhitelist::empty();
    whitelist.add(ShellRule::new("echo", vec![ArgPat::Any]));
    let queue = QueueApprover::new();
    queue.push(ApprovalVerdict::Approved);
    let approver: Arc<Mutex<Box<dyn Approver + Send>>> =
        Arc::new(Mutex::new(Box::new(queue.clone())));
    let (helper, client, mut s, root) =
        rig_with("shell", "t-shell", SCOPE_FIX, whitelist, approver).await;
    // 白名单命中：echo 审批放行后执行，输出含 hi
    let hit = call_tool(&mut s, 1, "shell_exec", SHELL_HIT).await;
    assert_eq!(hit["result"]["isError"], false, "{hit}");
    let hit_text = hit["result"]["content"][0]["text"].as_str().unwrap();
    assert!(hit_text.contains("hi"), "{hit_text}");
    // 闭集外：rm 非白名单命令拒绝并带原因（不 spawn）
    let miss = call_tool(&mut s, 2, "shell_exec", SHELL_MISS).await;
    assert_eq!(miss["result"]["isError"], true, "{miss}");
    let miss_text = miss["result"]["content"][0]["text"].as_str().unwrap();
    assert!(miss_text.contains("denied"), "{miss_text}");
    drain(s).await;
    teardown(&helper, &client, root);
}
