//! 集成测试：短命子命令仅 stderr（独立进程，进程内只 init 一次）。

use clap::Parser;

use p2p_cli::cli::Cli;

#[test]
fn discover_stays_on_stderr() {
    let command = Cli::try_parse_from(["p2p-cli", "discover"])
        .expect("parse argv")
        .command;
    assert!(
        p2p_cli::logging::init_for(&command).is_none(),
        "discover 是短命子命令，不落盘"
    );
}
