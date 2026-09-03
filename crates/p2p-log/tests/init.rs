//! 集成测试：init 契约——落盘成功路径、事件可见、重复初始化幂等。

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use p2p_log::{FileOptions, LogConfig, LogFormat};

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("p2p-log-it-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn init_contract_end_to_end() {
    std::env::set_var("RUST_LOG", "info");
    let dir = temp_dir("init");

    let first = p2p_log::init(LogConfig {
        format: LogFormat::Text,
        file: Some(FileOptions::with_default_caps(dir.clone(), "app.log")),
    });
    assert!(!first.already_initialized);
    assert!(first.fallback.is_none());
    assert_eq!(first.file_path.as_deref(), Some(dir.join("app.log").as_path()));

    tracing::info!(target: "p2p_cli", command = "node", "cli startup");
    let content = std::fs::read_to_string(dir.join("app.log")).expect("日志文件必须可读");
    assert!(content.contains("cli startup"), "启动事件必须落盘: {content}");
    assert!(content.contains("INFO"), "默认级别 info 须放行 info 事件: {content}");

    let second = p2p_log::init(LogConfig {
        format: LogFormat::Json,
        file: Some(FileOptions::with_default_caps(temp_dir("other"), "x.log")),
    });
    assert!(second.already_initialized, "重复初始化必须幂等");
    assert_eq!(second.file_path, first.file_path, "重复初始化返回首次结果");
    assert!(second.fallback.is_none());
}
