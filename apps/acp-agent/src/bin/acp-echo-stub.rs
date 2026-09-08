//! 测试专用回声子进程：stdin 行原样回写 stdout（Rust stdout 按行缓冲）。
//! --say-stderr        启动时向 stderr 写一行（stderr 接管落盘用例）
//! --print-cwd         启动时向 stdout 打印当前工作目录（cwd 监狱用例）
//! --emit-updates N MS 后台每 MS 毫秒发一条 session/update，共 N 条（0=无限；续连用例）
//! --session SID       update 的 sessionId（默认 s1）
//! --acp-agent         最小 ACP agent 模式（A2A2b 桥用）：initialize/session/new/
//!                     session/prompt 应答，每轮 prompt 回 --acp-chunks 条
//!                     agent_message_chunk 后以 stopReason=end_turn 结算
//! --acp-text S        chunk 文本前缀（默认 echo）
//! --prompt-hold MS    prompt 应答前延迟 MS 毫秒（working 态续聊/取消用例）
//! --acp-perm KIND     prompt 先发 request_permission（KIND），等应答后再继续
//!                     （应答 outcome=cancelled 时回 chunk "perm:cancelled"）
//! 行内含 acp-stub-exit 哨兵即静默退出（子进程退出断流用例）。

use std::io::{BufRead, BufReader, Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

struct Args {
    say_stderr: bool,
    print_cwd: bool,
    emit_enabled: bool,
    emit_updates: u64,
    emit_ms: u64,
    session: String,
    acp_agent: bool,
    acp_chunks: u64,
    acp_text: String,
    prompt_hold_ms: u64,
    acp_perm_kind: Option<String>,
}

fn main() {
    let args = parse_args();
    if args.say_stderr {
        eprintln!("acp-echo-stub: ready");
    }
    let out = Arc::new(Mutex::new(std::io::stdout()));
    if args.print_cwd {
        let cwd = std::env::current_dir()
            .map(|dir| dir.display().to_string())
            .unwrap_or_else(|err| format!("cwd-error:{err}"));
        write_line(&out, &cwd);
    }
    if args.emit_enabled {
        // count=0 表示无限发射（续连窗口蓄水用）
        spawn_emitter(out.clone(), &args);
    }
    if args.acp_agent {
        acp_agent_loop(&out, &args);
        return;
    }
    echo_loop(&out);
}

fn parse_args() -> Args {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut args = Args {
        say_stderr: false,
        print_cwd: false,
        emit_enabled: false,
        emit_updates: 0,
        emit_ms: 50,
        session: "s1".to_owned(),
        acp_agent: false,
        acp_chunks: 1,
        acp_text: "echo".to_owned(),
        prompt_hold_ms: 0,
        acp_perm_kind: None,
    };
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--say-stderr" => args.say_stderr = true,
            "--print-cwd" => args.print_cwd = true,
            "--emit-updates" => {
                args.emit_enabled = true;
                args.emit_updates = raw.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(0);
                args.emit_ms = raw.get(i + 2).and_then(|v| v.parse().ok()).unwrap_or(50);
                i += 2;
            }
            "--session" => {
                if let Some(sid) = raw.get(i + 1) {
                    args.session = sid.clone();
                }
                i += 1;
            }
            "--acp-agent" => args.acp_agent = true,
            "--acp-chunks" => {
                args.acp_chunks = raw.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(1);
                i += 1;
            }
            "--acp-text" => {
                if let Some(text) = raw.get(i + 1) {
                    args.acp_text = text.clone();
                }
                i += 1;
            }
            "--prompt-hold" => {
                args.prompt_hold_ms = raw.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(0);
                i += 1;
            }
            "--acp-perm" => {
                args.acp_perm_kind = raw.get(i + 1).cloned();
                i += 1;
            }
            other => eprintln!("acp-echo-stub: ignored arg {other}"),
        }
        i += 1;
    }
    args
}

/// 最小 ACP agent：按行处理请求；prompt 回合 = 延迟 +（可选权限交互）+ N 条
/// chunk + stopReason=end_turn。EOF 即退出（cancel/quiesce 经 stdin 关闭触发）。
/// 单一 BufReader 贯穿主循环与权限等待（双 reader 会在缓冲层丢行/死等）。
fn acp_agent_loop(out: &Arc<Mutex<std::io::Stdout>>, args: &Args) {
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    // while-let 逐次取行（不持有迭代器借用），prompt 回合内可再借同一 reader。
    while let Some(Ok(line)) = reader.by_ref().lines().next() {
        if line.contains("acp-stub-exit") {
            break;
        }
        let Ok(root) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        let (Some(id), Some(method)) = (
            root.get("id").filter(|v| !v.is_null()).cloned(),
            root.get("method").and_then(|m| m.as_str()).map(str::to_owned),
        ) else {
            continue; // 通知与残行不处理
        };
        match method.as_str() {
            "initialize" => reply(out, id, serde_json::json!({
                "protocolVersion": 1, "agentCapabilities": {}
            })),
            "session/new" => reply(out, id, serde_json::json!({ "sessionId": "s-a2a" })),
            "session/prompt" => run_prompt_turn(out, args, id, &mut reader),
            _ => reply(out, id, serde_json::json!({ "ignored": method })),
        }
    }
}

fn run_prompt_turn(
    out: &Arc<Mutex<std::io::Stdout>>,
    args: &Args,
    id: serde_json::Value,
    reader: &mut BufReader<std::io::StdinLock<'_>>,
) {
    if args.prompt_hold_ms > 0 {
        thread::sleep(Duration::from_millis(args.prompt_hold_ms));
    }
    if let Some(kind) = &args.acp_perm_kind {
        let perm_id = serde_json::json!(424_242);
        write_line(
            out,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": perm_id,
                "method": "session/request_permission",
                "params": {
                    "toolCall": { "kind": kind, "title": "stub" },
                    "options": [
                        { "optionId": "allow-once", "name": "Allow", "kind": "allow_once" },
                        { "optionId": "reject-once", "name": "Deny", "kind": "reject_once" }
                    ]
                }
            })
            .to_string(),
        );
        // 同一 reader 等权限应答；outcome=cancelled 时向 chunk 侧留可观测痕迹
        if wait_perm_denied(out, reader) {
            emit_chunk(out, &args.acp_text, "perm:cancelled");
        }
    }
    for n in 1..=args.acp_chunks {
        emit_chunk(out, &args.acp_text, &format!("chunk-{n}"));
    }
    reply(out, id, serde_json::json!({ "stopReason": "end_turn" }));
}

/// 等权限应答行；返回是否被拒（cancelled）。EOF 视为退出（返回 true 终止回合）。
fn wait_perm_denied(
    out: &Arc<Mutex<std::io::Stdout>>,
    reader: &mut BufReader<std::io::StdinLock<'_>>,
) -> bool {
    for line in reader.by_ref().lines() {
        let Ok(line) = line else { return true };
        let Ok(root) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        if root.get("id") == Some(&serde_json::json!(424_242)) {
            let denied = root
                .pointer("/result/outcome/outcome")
                .and_then(|v| v.as_str())
                .is_some_and(|o| o == "cancelled");
            let _ = out;
            return denied;
        }
    }
    true
}

fn emit_chunk(out: &Arc<Mutex<std::io::Stdout>>, prefix: &str, suffix: &str) {
    write_line(
        out,
        &serde_json::json!({
            "jsonrpc": "2.0", "method": "session/update",
            "params": { "sessionId": "s-a2a", "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": { "type": "text", "text": format!("{prefix}:{suffix}") } } }
        })
        .to_string(),
    );
}

fn reply(out: &Arc<Mutex<std::io::Stdout>>, id: serde_json::Value, result: serde_json::Value) {
    write_line(
        out,
        &serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string(),
    );
}

fn spawn_emitter(out: Arc<Mutex<std::io::Stdout>>, args: &Args) {
    let session = args.session.clone();
    let total = args.emit_updates;
    let pause = Duration::from_millis(args.emit_ms.max(1));
    thread::spawn(move || {
        let mut seq: u64 = 1;
        while total == 0 || seq <= total {
            let line = format!(
                "{{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{{\"sessionId\":\"{session}\",\"seq\":{seq}}}}}"
            );
            write_line(&out, &line);
            seq += 1;
            thread::sleep(pause);
        }
    });
}

fn echo_loop(out: &Arc<Mutex<std::io::Stdout>>) {
    let stdin = std::io::stdin();
    for line in BufReader::new(stdin.lock()).lines() {
        let Ok(line) = line else { break };
        if line.contains("acp-stub-exit") {
            break;
        }
        write_line(out, &line);
    }
}

fn write_line(out: &Mutex<std::io::Stdout>, line: &str) {
    let mut writer = out.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if writeln!(writer, "{line}").is_err() {
        return;
    }
    let _ = writer.flush();
}
