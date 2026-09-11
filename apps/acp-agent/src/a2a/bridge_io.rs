//! 桥的 ACP 面板 IO：握手（initialize/session/new）、请求写出、有界行读取。
//! 行护栏与既有 pump 同源（read_bounded_line），16 MiB 红线击穿即断。

use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};

use super::bridge::{BridgeParams, PromptCtx};
use crate::audit::AuditEvent;
use crate::pump::read_bounded_line;

/// 启动握手读等待上限：dsh 冷启（pnpm 装配）远慢于回声桩，取宽松护栏。
pub(crate) const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);

/// initialize + session/new；失败上抛由 run 审计并落 Failed 终态。
pub(crate) async fn handshake(
    ctx: &mut PromptCtx,
    reader: &mut BufReader<ChildStdout>,
    params: &BridgeParams,
) -> Result<(), String> {
    let init_id = ctx.take_id();
    write_request(
        &mut ctx.stdin,
        init_id,
        "initialize",
        json!({ "protocolVersion": 1, "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false } } }),
    )
    .await
    .map_err(|e| e.to_string())?;
    wait_response(reader, init_id, params).await?;
    let new_id = ctx.take_id();
    let cwd = params.cwd.as_ref().map(|p| p.display().to_string());
    write_request(&mut ctx.stdin, new_id, "session/new", json!({ "cwd": cwd }))
        .await
        .map_err(|e| e.to_string())?;
    let reply = wait_response(reader, new_id, params).await?;
    if reply
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(Value::as_str)
        .is_none()
    {
        return Err("session/new missing sessionId".into());
    }
    Ok(())
}

/// 等待指定 id 的应答行（跳过通知与无关 id）；超时即审计 SpawnFailed。
async fn wait_response(
    reader: &mut BufReader<ChildStdout>,
    id: u64,
    params: &BridgeParams,
) -> Result<Value, String> {
    let wait = async {
        loop {
            let line = read_line(reader)
                .await
                .map_err(|e| format!("guardrail: {e}"))?
                .ok_or_else(|| "child stdout eof during handshake".to_owned())?;
            let root: Value =
                serde_json::from_slice(&line).map_err(|e| format!("bad json: {e}"))?;
            if root.get("id") == Some(&json!(id))
                && (root.get("result").is_some() || root.get("error").is_some())
            {
                return Ok(root);
            }
        }
    };
    match tokio::time::timeout(HANDSHAKE_TIMEOUT, wait).await {
        Ok(result) => result.inspect_err(|_| {
            params.audit.record(AuditEvent::SpawnFailed {
                peer: params.peer.clone(),
                conn: params.task_id.clone(),
                detail: "a2a bridge handshake failed".into(),
            });
        }),
        Err(_) => Err(format!(
            "bridge handshake timed out after {}s",
            HANDSHAKE_TIMEOUT.as_secs()
        )),
    }
}

pub(crate) async fn write_request(
    stdin: &mut ChildStdin,
    id: u64,
    method: &str,
    params: Value,
) -> std::io::Result<()> {
    let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    let mut bytes = serde_json::to_vec(&line)?;
    bytes.push(b'\n');
    stdin.write_all(&bytes).await?;
    stdin.flush().await
}

/// 读一条 ndjson 行；None = 子进程 EOF（无残留半行）。
pub(crate) async fn read_line(
    reader: &mut BufReader<ChildStdout>,
) -> std::io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    let eof = read_bounded_line(reader, &mut line).await?;
    Ok(if eof { None } else { Some(line) })
}
