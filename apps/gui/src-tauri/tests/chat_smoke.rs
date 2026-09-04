//! chat 命令冒烟（T32，不起 webview）：真实节点 + 命令层双命令冒烟与校验矩阵。
//!
//! 覆盖：节点未启动时 chat 命令可读 Err；加好友（合法 peerId+addr）→ 列表可见；
//! 非法 peerId / 昵称超长 / addr 语法错 → 可读中文 Err；好友移除幂等；
//! chat_send 文本空/超长、媒体 mime 不匹配 → 可读中文 Err。临时目录测试结束清理。

use std::net::TcpListener;
use std::path::PathBuf;

use base64::Engine;
use p2p_chat::ChatKind;
use p2p_console::chat::{
    chat_friend_add, chat_friend_remove, chat_friends_list, chat_history, chat_send,
    ChatMediaInputJson,
};
use p2p_console::commands;
use p2p_console::state::AppState;
use p2p_console::types::{GuiConfig, NodeStatus};
use tauri::Manager;

/// 空闲端口探测（QUIC/TCP 共用近似；冒烟只取一个空闲端口段，无真实网络流量）。
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("绑定临时端口")
        .local_addr()
        .expect("读取端口")
        .port()
}

/// 冒烟临时目录（smoke 前缀，测试结束清理）。
fn smoke_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("smoke_p2p_gui_chat_{tag}_{}", std::process::id()))
}

/// 离线节点配置：mdns 关、无 bootstrap/relay，只回环不产生网络流量。
fn offline_config(dir: &std::path::Path) -> GuiConfig {
    GuiConfig {
        quic_port: free_port(),
        tcp_port: free_port(),
        enable_mdns: false,
        data_dir: dir.join("p2p-data").to_string_lossy().into_owned(),
        bootstrap: Vec::new(),
        relay_addrs: Vec::new(),
        advertised_addrs: Vec::new(),
        observation_port: None,
        observation_addrs: Vec::new(),
    }
}

fn valid_peer_id() -> String {
    bs58::encode([42u8; 32]).into_string()
}

fn illegal_peer_id() -> String {
    "!!!not-base58!!!".to_string()
}

fn wrong_length_peer_id() -> String {
    bs58::encode([1u8; 16]).into_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chat_smoke_add_list_remove_and_validation_matrix() {
    let dir = smoke_dir("basic");
    let _guard = DirGuard(dir.clone());
    std::fs::create_dir_all(&dir).expect("创建冒烟数据目录");

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    handle.manage(AppState::new(dir.clone()));
    let state: tauri::State<'_, AppState> = handle.state();

    // 0. 节点未启动：chat 命令返回可读 Err（禁止静默）
    let not_started = chat_friends_list(state.clone())
        .await
        .expect_err("节点未启动时必须 Err");
    assert!(not_started.contains("节点未运行"), "实际: {not_started}");

    // 1. 启动离线节点
    let status: NodeStatus =
        commands::node_start(handle.clone(), state.clone(), offline_config(dir.as_path()))
            .await
            .expect("节点启动");
    assert!(status.running);

    // 2. 冒烟：加好友（合法 peerId+addr）→ 列表可见
    let peer = valid_peer_id();
    let friend = chat_friend_add(
        state.clone(),
        peer.clone(),
        " 冒烟好友 ".to_string(),
        vec!["127.0.0.1/u3400".to_string()],
    )
    .await
    .expect("合法参数加好友必须成功");
    assert_eq!(friend.peer_id, peer);
    assert_eq!(friend.nickname, "冒烟好友", "nickname 应 trim");
    assert_eq!(friend.note, None);

    let list = chat_friends_list(state.clone()).await.expect("好友列表");
    assert!(
        list.iter().any(|f| f.peer_id == peer),
        "好友簿应含刚加入的节点: {list:?}"
    );

    // 3. 校验矩阵：非法 peerId / 昵称超长 / addr 语法错 → 可读中文 Err
    let bad = chat_friend_add(
        state.clone(),
        illegal_peer_id(),
        "昵称".to_string(),
        Vec::new(),
    )
    .await
    .expect_err("非法 peerId 必须 Err");
    assert!(bad.contains("base58"), "实际: {bad}");

    let short = chat_friend_add(
        state.clone(),
        wrong_length_peer_id(),
        "昵称".to_string(),
        Vec::new(),
    )
    .await
    .expect_err("长度非 32 字节的 peerId 必须 Err");
    assert!(short.contains("长度"), "实际: {short}");

    let long_nick = chat_friend_add(state.clone(), valid_peer_id(), "x".repeat(65), Vec::new())
        .await
        .expect_err("昵称超 64 字符必须 Err");
    assert!(long_nick.contains("昵称"), "实际: {long_nick}");

    let bad_addr = chat_friend_add(
        state.clone(),
        valid_peer_id(),
        "昵称".to_string(),
        vec!["no-slash/u1".to_string()],
    )
    .await
    .expect_err("addr 语法非法必须 Err");
    assert!(bad_addr.contains("地址"), "实际: {bad_addr}");

    // 4. chat_send 校验矩阵：文本空/超长、媒体 mime 不匹配 → 可读中文 Err
    let empty_text = chat_send(
        state.clone(),
        peer.clone(),
        ChatKind::Text,
        Some("   ".to_string()),
        None,
        None,
    )
    .await
    .expect_err("空白文本必须 Err");
    assert!(empty_text.contains("文本"), "实际: {empty_text}");

    let over_text = chat_send(
        state.clone(),
        peer.clone(),
        ChatKind::Text,
        Some("a".repeat(2001)),
        None,
        None,
    )
    .await
    .expect_err("超长文本必须 Err");
    assert!(over_text.contains("上限"), "实际: {over_text}");

    let mime_mismatch = chat_send(
        state.clone(),
        peer.clone(),
        ChatKind::Image,
        None,
        Some(ChatMediaInputJson {
            name: "a.txt".into(),
            mime: "text/plain".into(),
            data_base64: base64::engine::general_purpose::STANDARD.encode(b"x"),
        }),
        None,
    )
    .await
    .expect_err("image kind 配 text/plain 必须 Err");
    assert!(mime_mismatch.contains("MIME"), "实际: {mime_mismatch}");

    // 5. chat_friend_remove 幂等 + 历史空数组
    assert!(
        chat_friend_remove(state.clone(), peer.clone())
            .await
            .expect("移除好友"),
        "在簿好友移除返回 true"
    );
    assert!(
        !chat_friend_remove(state.clone(), peer.clone())
            .await
            .expect("重复移除"),
        "幂等：不再在簿返回 false"
    );
    let history = chat_history(state.clone(), peer.clone(), None, None)
        .await
        .expect("历史查询");
    assert!(
        history.is_empty(),
        "未发送成功过消息，历史应为空: {history:?}"
    );

    // 6. 收尾
    let stopped = commands::node_stop(handle.clone(), state.clone())
        .await
        .expect("停止节点");
    assert!(!stopped.running);
}

/// 退出清理：目录删不掉留告警，不 panic（避免掩盖真失败原因）。
struct DirGuard(PathBuf);

impl Drop for DirGuard {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("[chat-smoke] 清理临时目录失败 {}: {e}", self.0.display());
            }
        }
    }
}
