//! A2A2b task 链 E2E（a2a-over-p2p-design §11 验收）：真 Node QUIC loopback
//! 双节点——task create/send → 桥驱动 stub 子进程产出 → message 帧回流 →
//! status 终态 → cancel → cancelled；非授权 peer 访问私有 agent gate-denied；
//! FilePart 上行显式拒绝；每 peer 并发 task ≤4。

mod task_wave_common;

use a2a::{FilePart, Message, Part, Role, TaskRequest};
use p2p::Node;
use serde_json::{json, Value};
use task_wave_common::{
    as_response, create_request, drive_to_terminal, guest_stream, host_rig, is_notice, read_json,
    send_request,
};

async fn guest_with_public_agent(
    tag: &str,
    stub_args: &[&str],
) -> (task_wave_common::HostRig, Node, p2p::BoxedStream) {
    let host = host_rig(tag, stub_args).await;
    host.agents
        .create(
            Some("pub-agent".into()),
            "公开评审".into(),
            "d".into(),
            vec![],
            a2a::Visibility::Public,
            1,
        )
        .expect("create public");
    let guest = Node::builder().mdns(false).build().await.expect("guest");
    let stream = guest_stream(&guest, &host.peer, &host.node.listen_addrs()).await;
    (host, guest, stream)
}

#[tokio::test]
async fn t1_public_task_roundtrip_and_snapshot() {
    let (_host, guest, mut stream) =
        guest_with_public_agent("t1", &["--acp-agent", "--acp-chunks", "2"]).await;
    send_request(&mut stream, &create_request("pub-agent", "你好", 1)).await;
    let reply = read_json(&mut stream).await;
    let (result, error) = as_response(&reply);
    assert!(error.is_none(), "create 失败: {reply:?}");
    let task_id = result
        .and_then(|r| r.get("taskId"))
        .and_then(Value::as_str)
        .expect("taskId")
        .to_owned();
    let (statuses, texts) = drive_to_terminal(&mut stream).await;
    assert_eq!(statuses.first().map(String::as_str), Some("working"));
    assert_eq!(statuses.last().map(String::as_str), Some("completed"));
    assert_eq!(texts, vec!["echo:chunk-1", "echo:chunk-2"], "message 回流");
    // tasks/get 快照：终态权威 + 消息序（user 首条 + agent 产出）
    send_request(&mut stream, &TaskRequest::get(&task_id, 2)).await;
    let snap = read_json(&mut stream).await;
    let (result, _) = as_response(&snap);
    let result = result.expect("snapshot result");
    assert_eq!(
        result.get("state").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        result
            .pointer("/messages/0/parts/0/text")
            .and_then(Value::as_str),
        Some("你好")
    );
    assert_eq!(
        result
            .pointer("/messages/1/parts/0/text")
            .and_then(Value::as_str),
        Some("echo:chunk-1")
    );
    guest.shutdown();
}

#[tokio::test]
async fn t2_send_while_working_appends_and_completes() {
    let (_host, guest, mut stream) =
        guest_with_public_agent("t2", &["--acp-agent", "--prompt-hold", "300"]).await;
    send_request(&mut stream, &create_request("pub-agent", "一", 1)).await;
    let reply = read_json(&mut stream).await;
    let task_id = reply
        .pointer("/result/taskId")
        .and_then(Value::as_str)
        .expect("taskId")
        .to_owned();
    let working = read_json(&mut stream).await;
    assert!(is_notice(&working, "tasks/status"), "{working:?}");
    // working 态续聊：send 应答 + 两轮产出 + completed
    send_request(
        &mut stream,
        &TaskRequest::send(&task_id, Message::user_text("二"), 2),
    )
    .await;
    let send_reply = read_json(&mut stream).await;
    assert!(
        send_reply.get("result").is_some(),
        "send 失败: {send_reply:?}"
    );
    let (statuses, texts) = drive_to_terminal(&mut stream).await;
    assert_eq!(statuses.last().map(String::as_str), Some("completed"));
    assert_eq!(texts, vec!["echo:chunk-1", "echo:chunk-1"]);
    guest.shutdown();
}

#[tokio::test]
async fn t3_cancel_quiesces_and_reports_cancelled() {
    let (host, guest, mut stream) =
        guest_with_public_agent("t3", &["--acp-agent", "--prompt-hold", "30000"]).await;
    send_request(&mut stream, &create_request("pub-agent", "长任务", 1)).await;
    let reply = read_json(&mut stream).await;
    let task_id = reply
        .pointer("/result/taskId")
        .and_then(Value::as_str)
        .expect("taskId")
        .to_owned();
    let working = read_json(&mut stream).await;
    assert!(is_notice(&working, "tasks/status"), "{working:?}");
    send_request(&mut stream, &TaskRequest::cancel(&task_id, 2)).await;
    let cancel_reply = read_json(&mut stream).await;
    assert!(
        cancel_reply.get("result").is_some(),
        "cancel 失败: {cancel_reply:?}"
    );
    let (statuses, _) = drive_to_terminal(&mut stream).await;
    assert_eq!(statuses.last().map(String::as_str), Some("cancelled"));
    assert!(
        host.audit
            .contains(|e| matches!(e, acp_agent::AuditEvent::A2aTaskCancelled { .. })),
        "cancel 必须留审计"
    );
    // 快照确认终态可查（服务端驻留）
    send_request(&mut stream, &TaskRequest::get(&task_id, 3)).await;
    let snap = read_json(&mut stream).await;
    assert_eq!(
        snap.pointer("/result/state").and_then(Value::as_str),
        Some("cancelled")
    );
    guest.shutdown();
}

#[tokio::test]
async fn t4_private_agent_gate_denies_uninvited_peer() {
    let host = host_rig("t4", &["--acp-agent"]).await;
    host.agents
        .create(
            Some("priv-agent".into()),
            "私有助理".into(),
            "d".into(),
            vec![],
            a2a::Visibility::Private,
            1,
        )
        .expect("create private");
    let guest = Node::builder().mdns(false).build().await.expect("guest");
    let mut stream = guest_stream(&guest, &host.peer, &host.node.listen_addrs()).await;
    send_request(&mut stream, &create_request("priv-agent", "你好", 1)).await;
    let reply = read_json(&mut stream).await;
    let error = reply.get("error").expect("必须被拒绝");
    assert!(
        error
            .get("message")
            .and_then(Value::as_str)
            .is_some_and(|m| m.contains("gate-denied")),
        "{error:?}"
    );
    assert!(
        host.audit
            .contains(|e| matches!(e, acp_agent::AuditEvent::A2aTaskDenied { .. })),
        "门禁拒绝必须留审计"
    );
    guest.shutdown();
}

#[tokio::test]
async fn t5_file_part_upload_rejected_explicitly() {
    let (_host, guest, mut stream) = guest_with_public_agent("t5", &["--acp-agent"]).await;
    let file_message = Message {
        role: Role::User,
        parts: vec![Part::File(FilePart {
            name: "x.png".into(),
            mime_type: "image/png".into(),
            bytes: "aGk=".into(),
        })],
    };
    send_request(
        &mut stream,
        &TaskRequest::new(
            "tasks/create",
            1,
            json!({ "agentId": "pub-agent", "message": file_message }),
        ),
    )
    .await;
    let reply = read_json(&mut stream).await;
    let error = reply.get("error").expect("FilePart 上行必须显式拒绝");
    assert!(
        error.get("message").and_then(Value::as_str).is_some(),
        "{error:?}"
    );
    guest.shutdown();
}

#[tokio::test]
async fn t6_per_peer_concurrent_task_cap() {
    let host = host_rig("t6", &["--acp-agent", "--prompt-hold", "30000"]).await;
    host.agents
        .create(
            Some("pub-agent".into()),
            "公开评审".into(),
            "d".into(),
            vec![],
            a2a::Visibility::Public,
            1,
        )
        .expect("create public");
    let guest = Node::builder().mdns(false).build().await.expect("guest");
    let addrs = host.node.listen_addrs();
    let mut streams = Vec::new();
    for n in 0..4u64 {
        let mut stream = guest_stream(&guest, &host.peer, &addrs).await;
        send_request(&mut stream, &create_request("pub-agent", "占坑", n + 1)).await;
        let reply = read_json(&mut stream).await;
        assert!(
            reply.get("result").is_some(),
            "第 {} 个 task 应成功: {reply:?}",
            n + 1
        );
        streams.push(stream);
    }
    // 第 5 条流：超过每 peer 并发上限 → task-cap 拒绝
    let mut fifth = guest_stream(&guest, &host.peer, &addrs).await;
    send_request(&mut fifth, &create_request("pub-agent", "超限", 99)).await;
    let reply = read_json(&mut fifth).await;
    let error = reply.get("error").expect("第 5 个 task 必须被限流");
    assert!(
        error
            .get("message")
            .and_then(Value::as_str)
            .is_some_and(|m| m.contains("task-cap")),
        "{error:?}"
    );
    drop(streams); // 流落体即关（1 task=1 流，关流即断）
    guest.shutdown();
}
