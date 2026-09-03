//! 集成测试：落盘初始化失败必须回退 stderr 并留告警（不静默红线）。

use std::time::{SystemTime, UNIX_EPOCH};

use p2p_log::{FileOptions, LogConfig, LogFormat};

#[test]
fn broken_file_dir_falls_back_to_stderr_with_warning() {
    std::env::set_var("RUST_LOG", "info");
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("p2p-log-it-fb-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    // 用一个已存在的普通文件充当父目录：create_dir_all 必然失败
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, b"x").unwrap();

    let report = p2p_log::init(LogConfig {
        format: LogFormat::Text,
        file: Some(FileOptions::with_default_caps(blocker.join("sub"), "app.log")),
    });

    assert!(report.file_path.is_none(), "失败路径不得伪造文件路径");
    let msg = report.fallback.expect("初始化失败必须留告警（不静默红线）");
    assert!(msg.contains("回退 stderr"), "告警必须说明回退行为: {msg}");
    assert!(msg.contains("日志落盘初始化失败"), "告警必须说明失败事实: {msg}");
    std::fs::remove_dir_all(&dir).ok();
}
