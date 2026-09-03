//! 集成测试：JSON 输出格式落盘。

use std::time::{SystemTime, UNIX_EPOCH};

use p2p_log::{FileOptions, LogConfig, LogFormat};

#[test]
fn json_format_writes_parseable_events() {
    std::env::set_var("RUST_LOG", "info");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("p2p-log-it-json-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let report = p2p_log::init(LogConfig {
        format: LogFormat::Json,
        file: Some(FileOptions::with_default_caps(dir.clone(), "app.log")),
    });
    assert!(report.file_path.is_some());

    tracing::info!(target: "p2p_json_test", peer = "abc", "json event marker");
    let content = std::fs::read_to_string(dir.join("app.log")).expect("日志文件必须可读");
    assert!(
        content.contains("json event marker"),
        "事件文本必须落盘: {content}"
    );
    for line in content.lines().filter(|l| !l.trim().is_empty()) {
        let trimmed = line.trim_start();
        assert!(trimmed.starts_with('{'), "每行必须是 JSON 对象: {line}");
        assert!(
            line.trim_end().ends_with('}'),
            "每行必须是完整 JSON 对象: {line}"
        );
        assert!(
            line.contains(r#""level":"INFO""#),
            "JSON 必须带 level 字段: {line}"
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}
