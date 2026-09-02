//! 生命周期单测：仅本地回环装配（随机端口、mdns 关、无 bootstrap/relay），不产生真实网络流量。

use super::*;

/// 独立临时目录，结束清理。
fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-console-{}-{}", tag, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("创建临时目录");
    dir
}

fn offline_config(state: &AppState) -> GuiConfig {
    GuiConfig {
        enable_mdns: false,
        ..state.config_get()
    }
}

#[tokio::test]
async fn duplicate_node_start_returns_err() {
    let dir = temp_root("dup-start");
    let state = AppState::new(dir.join("app"));
    let cfg = offline_config(&state);

    let started = state.start(cfg.clone()).await.expect("首次启动成功");
    assert!(started.status.running);
    assert!(started.status.peer_id.is_some());
    assert!(!started.status.listen_addrs.is_empty());

    let err = state.start(cfg).await.expect_err("重复启动必须失败");
    assert!(err.contains("已在运行"), "实际错误: {err}");

    assert!(state.stop().await, "停掉运行中的节点");
    assert!(!state.stop().await, "stop 幂等");
    let status = state.status().await;
    assert!(!status.running);
    assert_eq!(status.peer_id, None);

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn status_without_node_uses_persisted_config() {
    let dir = temp_root("status-fallback");
    let state = AppState::new(dir.join("app"));
    let persisted = state.config_get();

    let status = state.status().await;
    assert!(!status.running);
    assert_eq!(status.peer_id, None);
    assert_eq!(status.started_at_ms, None);
    assert_eq!(status.listen_addrs, Vec::<String>::new());
    assert_eq!(status.config, persisted, "未运行回持久化配置");

    assert_eq!(state.metrics().await, MetricsJson::default());
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn dial_and_ping_require_running_node() {
    let dir = temp_root("not-running");
    let state = AppState::new(dir.join("app"));
    let peer = bs58::encode([9u8; 32]).into_string();

    let dial_err = state
        .dial(&format!("{peer}@127.0.0.1/3400"))
        .await
        .unwrap_err();
    assert!(dial_err.contains("节点未运行"));
    let ping_err = state.ping(&peer, 1000).await.unwrap_err();
    assert!(ping_err.contains("节点未运行"));
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn identity_reset_stops_node_and_deletes_seed() {
    let dir = temp_root("identity-reset");
    let app = dir.join("app");
    let data = app.join("p2p-data");
    fs::create_dir_all(&data).expect("创建数据目录");
    fs::write(data.join("key.seed"), [0u8; 32]).expect("写入种子");
    let state = AppState::new(app);
    let mut cfg = offline_config(&state);
    cfg.data_dir = data.to_string_lossy().into_owned();
    state.config.save(&cfg).expect("保存配置");

    state.start(cfg).await.expect("启动成功");
    let (status, was_running) = state.reset_identity().await.expect("重置身份");
    assert!(was_running, "重置停掉了运行中的节点");
    assert!(!status.running);
    assert!(!data.join("key.seed").exists(), "种子文件已被删除");

    let (_, was_running) = state.reset_identity().await.expect("再次重置");
    assert!(!was_running, "未运行时重置不产生停机事件");
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn config_save_does_not_touch_running_node() {
    let dir = temp_root("save-while-running");
    let state = AppState::new(dir.join("app"));
    let cfg = offline_config(&state);
    state.start(cfg.clone()).await.expect("启动成功");

    let mut new_cfg = cfg.clone();
    new_cfg.quic_port = 12345;
    let saved = state.config_save(new_cfg).expect("保存配置");
    assert_eq!(saved.quic_port, 12345);
    assert_eq!(state.config_get().quic_port, 12345);

    let status = state.status().await;
    assert!(status.running, "节点不受 config_save 影响");
    assert_eq!(status.config.quic_port, cfg.quic_port, "生效配置不变");
    state.stop().await;
    let _ = fs::remove_dir_all(&dir);
}
