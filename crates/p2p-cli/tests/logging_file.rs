//! 集成测试：常驻子命令默认文件落盘（独立进程，进程内只 init 一次）。

use clap::Parser;

use p2p_cli::cli::{Cli, Command};

fn command_of(argv: &[&str]) -> Command {
    Cli::try_parse_from(argv).expect("parse argv").command
}

#[test]
fn node_defaults_to_file_logging() {
    let path = p2p_cli::logging::init_for(&command_of(&["p2p-cli", "node", "--data", "d"]))
        .expect("node 必须默认文件落盘");
    assert!(
        path.to_string_lossy().ends_with("node.log"),
        "路径应指向 node.log: {path:?}"
    );
    assert!(path.exists(), "日志文件必须真实产生: {path:?}");

    tracing::info!(target: "p2p_cli", "cli startup smoke");
    let content = std::fs::read_to_string(&path).expect("日志文件必须可读");
    assert!(
        content.contains("cli startup smoke"),
        "事件必须落盘: {content}"
    );
}
