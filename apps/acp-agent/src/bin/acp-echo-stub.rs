//! 测试专用回声子进程：stdin 行原样回写 stdout（Rust stdout 按行缓冲）。
//! --say-stderr        启动时向 stderr 写一行（stderr 接管落盘用例）
//! --print-cwd         启动时向 stdout 打印当前工作目录（cwd 监狱用例）
//! --emit-updates N MS 后台每 MS 毫秒发一条 session/update，共 N 条（0=无限；续连用例）
//! --session SID       update 的 sessionId（默认 s1）
//! 行内含 acp-stub-exit 哨兵即静默退出（子进程退出断流用例）。

use std::io::{BufRead, BufReader, Write};
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
            other => eprintln!("acp-echo-stub: ignored arg {other}"),
        }
        i += 1;
    }
    args
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
