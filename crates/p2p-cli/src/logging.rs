//! 子命令感知的日志装配（E7-L1）：node/bootstrap 默认滚动文件落盘，其余 stderr。
//!
//! 落盘目录用平台标准日志目录（见 p2p_log::default_log_dir）；日志文件路径打印
//! 到 stderr 供脚本采集。初始化失败不阻断启动——p2p-log 内部回退 stderr 并留告警。

use std::path::PathBuf;

use p2p_log::{FileOptions, LogConfig, LogFormat};

use crate::cli::Command;

/// 按子命令装配日志：常驻子命令（node/bootstrap）默认文件落盘并返回路径。
///
/// 全局订阅器只能安装一次（幂等回传首次报告），因此选择逻辑独立在
/// [config_for]（纯函数），全局行为由 tests/logging_init.rs 单进程验证。
pub fn init_for(command: &Command) -> Option<PathBuf> {
    let report = p2p_log::init(config_for(command));
    if let Some(path) = &report.file_path {
        eprintln!("p2p-cli: 日志文件 {}", path.display());
    }
    if let Some(fallback) = &report.fallback {
        eprintln!("p2p-cli: {fallback}");
    }
    report.file_path
}

/// 子命令到日志配置的选择：node/bootstrap 落盘，ping/discover 仅 stderr。
pub fn config_for(command: &Command) -> LogConfig {
    match command {
        Command::Bootstrap(_) => file_config("bootstrap"),
        Command::Node(_) => file_config("node"),
        Command::Ping(_) | Command::Discover(_) => LogConfig::default(),
    }
}

/// 常驻子命令的落盘配置：平台标准日志目录 / <子命令>.log；目录无法定位退临时目录。
fn file_config(name: &str) -> LogConfig {
    let dir = p2p_log::default_log_dir("p2p-cli")
        .unwrap_or_else(|| std::env::temp_dir().join("p2p-cli"));
    LogConfig {
        format: LogFormat::Text,
        file: Some(FileOptions::with_default_caps(dir, format!("{name}.log"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn config_of(argv: &[&str]) -> LogConfig {
        let cli = crate::cli::Cli::try_parse_from(argv).expect("parse argv");
        config_for(&cli.command)
    }

    #[test]
    fn resident_commands_select_file_logging() {
        let node = config_of(&["p2p-cli", "node", "--data", "d"]);
        let file = node.file.expect("node 必须选择文件落盘");
        assert!(file.name.starts_with("node.log"), "文件名带子命令: {:?}", file.name);

        let bootstrap = config_of(&["p2p-cli", "bootstrap", "--data", "d"]);
        let file = bootstrap.file.expect("bootstrap 必须选择文件落盘");
        assert!(file.name.starts_with("bootstrap.log"), "文件名带子命令: {:?}", file.name);
    }

    #[test]
    fn short_lived_commands_select_stderr_only() {
        let peer64 = "a".repeat(64);
        for argv in [
            vec!["p2p-cli", "ping", peer64.as_str()],
            vec!["p2p-cli", "discover"],
        ] {
            assert!(
                config_of(&argv).file.is_none(),
                "{argv:?} 是短命子命令，不落盘"
            );
        }
    }

    #[test]
    fn file_config_targets_platform_log_dir() {
        let node = config_of(&["p2p-cli", "node", "--data", "d"]);
        let file = node.file.unwrap();
        assert!(!file.dir.as_os_str().is_empty(), "目录必须有值");
        assert_eq!(file.dir.file_name().map(|s| s.to_string_lossy()), Some("p2p-cli".into()));
    }
}
