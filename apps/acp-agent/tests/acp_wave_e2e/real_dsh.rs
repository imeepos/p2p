//! 真 dsh 双进程链路用例（`#[ignore]`，验收 `--ignored --test-threads 1` 单独跑）：
//! 子进程 = 生产命令 dsh --profile acp（argv 可用 ACP_E2E_REAL_DSH 覆盖），覆盖
//! 握手+initialize roundtrip 与 session/new(敌意 mcpServers) 剥离后 roundtrip。
//! dsh 不可用（spawn 失败 / initialize 无应答 / 应答不可判读）→ 直写 stderr 留
//! `SKIP:` 信号后结束，绝不假绿；dsh 应答之后的断言失败是真实回归，照常红。

use acp_common::{AskRoute, Scope, ServerHello};
use serde_json::Value;

use acp_agent::AuditEvent;
use p2p::BoxedStream;

use crate::common::{
    handshake_client, open_stream, rig, send_line, shutdown, test_grant_full, Rig,
};
use crate::{line_within, skip_signal};

const PROBE_SECS: u64 = 30;

fn real_dsh_command() -> Vec<String> {
    std::env::var("ACP_E2E_REAL_DSH")
        .unwrap_or_else(|_| "dsh --profile acp".to_owned())
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

async fn real_rig(tag: &str) -> Rig {
    let cmd = real_dsh_command();
    rig(
        tag,
        test_grant_full(Scope::Sandbox, Vec::new(), AskRoute::RemoteGui),
        |cfg| {
            cfg.command = cmd.clone();
            // 真 dsh 启动远慢于桩：宽限与应答预算放宽
            cfg.grace_secs = 15;
            cfg.permission_timeout_secs = 25;
        },
    )
    .await
}

/// 可用性探测：握手 ready 且 initialize 拿到 result 应答才算本机 dsh 可用。
/// 返回 None = 不可用（SKIP 原因已直写 stderr）。
async fn probe(r: &Rig) -> Option<(BoxedStream, String)> {
    let mut stream = open_stream(r).await;
    let ServerHello::Ready { ready } = handshake_client(&mut stream).await else {
        skip_signal("handshake denied (dsh spawn failed or policy rejected)");
        return None;
    };
    assert!(ready.ticket.is_some(), "fresh conn must carry a ticket");
    send_line(
        &mut stream,
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"clientCapabilities\":{}}}",
    )
    .await;
    loop {
        let Some(line) = line_within(&mut stream, PROBE_SECS).await else {
            skip_signal("no initialize answer (dsh exited early or lacks --profile acp)");
            return None;
        };
        let Ok(root) = serde_json::from_str::<Value>(line.trim_end()) else {
            continue;
        };
        if root.get("id").and_then(Value::as_i64) == Some(1) {
            if root.get("result").is_some_and(Value::is_object) {
                return Some((stream, line));
            }
            skip_signal(&format!("initialize not answerable on this host: {line}"));
            return None;
        }
    }
}

#[ignore = "真链路用例：需要本机 dsh --profile acp；验收命令以 --ignored 单独跑"]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_dsh_handshake_initialize_roundtrip() {
    let r = real_rig("wave-d1").await;
    let Some((stream, answer)) = probe(&r).await else {
        shutdown(&r);
        return;
    };
    let root: Value = serde_json::from_str(answer.trim_end()).expect("json");
    assert_eq!(root["jsonrpc"], "2.0", "{answer}");
    assert!(
        root["result"].is_object(),
        "initialize result must be an ACP capabilities object: {answer}",
    );
    drop(stream);
    shutdown(&r);
}

#[ignore = "真链路用例：需要本机 dsh --profile acp；验收命令以 --ignored 单独跑"]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_dsh_session_new_strips_hostile_mcp_and_roundtrips() {
    let r = real_rig("wave-d2").await;
    let Some((mut stream, _)) = probe(&r).await else {
        shutdown(&r);
        return;
    };
    let cwd = std::env::temp_dir().to_string_lossy().into_owned();
    let session_new = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "session/new",
        "params": {
            "cwd": cwd,
            "mcpServers": [{ "command": "evil", "args": ["-c", "boom"] }],
        },
    })
    .to_string();
    send_line(&mut stream, &session_new).await;
    // id==2 的应答即证明请求经剥离后穿越真 dsh roundtrip（期间夹杂的通知跳过）
    loop {
        let Some(line) = line_within(&mut stream, PROBE_SECS).await else {
            panic!("session/new must be answered by the real dsh within budget");
        };
        let Ok(root) = serde_json::from_str::<Value>(line.trim_end()) else {
            continue;
        };
        if root.get("id").and_then(Value::as_i64) == Some(2) {
            break;
        }
    }
    assert!(
        r.audit.contains(|ev| matches!(
            ev,
            AuditEvent::McpRewritten { action, .. } if action == "stripped"
        )),
        "hostile mcpServers must be stripped at the bridge: {:?}",
        r.audit.snapshot(),
    );
    shutdown(&r);
}
