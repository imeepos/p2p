//! 测试专用回声子进程：stdin 行原样回写 stdout（Rust stdout 按行缓冲）。
//! 行内含 acp-stub-exit 哨兵即静默退出（供子进程退出断流用例）；
//! --say-stderr 启动时向 stderr 写一行（供 stderr 接管落盘用例）。

use std::io::{BufRead, BufReader, Write};

const EXIT_SENTINEL: &str = "acp-stub-exit";

fn main() {
    if std::env::args().any(|arg| arg == "--say-stderr") {
        eprintln!("acp-echo-stub: ready");
    }
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in BufReader::new(stdin.lock()).lines() {
        let Ok(line) = line else { break };
        if line.contains(EXIT_SENTINEL) {
            break;
        }
        if writeln!(out, "{line}").is_err() {
            break;
        }
        if out.flush().is_err() {
            break;
        }
    }
}
