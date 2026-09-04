//! chat 命令层增量边界（IM-T49 采纳自 feat/t36-chat-boundary-tests 11fe0a6）：
//! 只补 main 现有 chat_smoke/chat_contract 未覆盖的命令层缺口——
//! kind×media 配对校验、base64 非法、自发自收、ChatMediaFileJson 输出键名，
//! 以及双回环真节点经命令层的成功投递（含 replyTo 透传）。

use std::path::{Path, PathBuf};

use base64::Engine;
use p2p_chat::{ChatKind, ChatStatus};
use p2p_console::chat::{
    chat_friend_invite, chat_history, chat_invite_accept, chat_invites_list, chat_send,
    ChatMediaFileJson, ChatMediaInputJson,
};
use p2p_console::commands;
use p2p_console::state::AppState;
use p2p_console::types::GuiConfig;
use serde_json::json;
use tauri::Manager;

/// 回环离线配置：端口 0 内核动态分配（T44，固定端口有 AddrInUse 假红）；
/// 回环占位阻断出厂云端端点回退，不启动任何外部服务。
fn loopback_config(dir: &Path) -> GuiConfig {
    GuiConfig {
        quic_port: 0,
        tcp_port: 0,
        enable_mdns: false,
        data_dir: dir.join("p2p-data").to_string_lossy().into_owned(),
        bootstrap: vec!["127.0.0.1/u1".into()],
        relay_addrs: vec!["127.0.0.1/u3403".into()],
        advertised_addrs: Vec::new(),
        observation_port: None,
        observation_addrs: vec!["127.0.0.1:3402".into()],
    }
}

fn cmd_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("chat-cmd-matrix-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

/// 退出清理：目录删不掉留告警，不 panic（避免掩盖真失败原因）。
struct DirGuard(PathBuf);

impl Drop for DirGuard {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            eprintln!(
                "[chat-cmd-matrix] 清理临时目录失败 {}: {e}",
                self.0.display()
            );
        }
    }
}

/// 错误矩阵行：（用例名, kind, text, media, peer, 期望错误片段）。
type SendErrCase = (
    &'static str,
    ChatKind,
    Option<String>,
    Option<ChatMediaInputJson>,
    String,
    &'static str,
);

fn media(mime: &str, data: &[u8]) -> ChatMediaInputJson {
    ChatMediaInputJson {
        name: "a.bin".into(),
        mime: mime.into(),
        data_base64: base64::engine::general_purpose::STANDARD.encode(data),
    }
}

fn bad_b64() -> ChatMediaInputJson {
    ChatMediaInputJson {
        name: "a.bin".into(),
        mime: "application/octet-stream".into(),
        data_base64: "!!!not-base64!!!".into(),
    }
}

/// 命令层独有校验缺口：text null 语义、kind×media 配对、自发自收、base64 非法。
/// （空白/超长文本与 MIME 错配已由 chat_smoke 覆盖，不重复。）
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_command_rejects_pairing_base64_and_self() {
    let dir = cmd_dir("pair");
    let _guard = DirGuard(dir.clone());
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    handle.manage(AppState::new(dir.clone()));
    let state = handle.state::<AppState>();
    commands::node_start(handle.clone(), state.clone(), loopback_config(&dir))
        .await
        .expect("启动回环离线节点");
    let status = commands::node_status(state.clone())
        .await
        .expect("状态快照");
    let own = status.peer_id.expect("peer_id 必有");
    let remote = bs58::encode([42u8; 32]).into_string();
    let cases: Vec<SendErrCase> = vec![
        (
            "text 参数 null 视为空",
            ChatKind::Text,
            None,
            None,
            remote.clone(),
            "文本为空",
        ),
        (
            "text 带附件拒绝",
            ChatKind::Text,
            Some("hi".into()),
            Some(media("application/octet-stream", b"x")),
            remote.clone(),
            "不能携带附件",
        ),
        (
            "image 缺附件拒绝",
            ChatKind::Image,
            None,
            None,
            remote.clone(),
            "必须携带附件",
        ),
        (
            "自发自收拒绝",
            ChatKind::Text,
            Some("hi".into()),
            None,
            own,
            "自己",
        ),
        (
            "base64 非法拒绝",
            ChatKind::File,
            None,
            Some(bad_b64()),
            remote,
            "base64",
        ),
    ];
    for (what, kind, text, input, peer, expect) in cases {
        let err = chat_send(state.clone(), peer, kind, text, input, None)
            .await
            .expect_err(what);
        assert!(err.contains(expect), "{what}: 实际 {err}");
    }
    commands::node_stop(handle.clone(), state.clone())
        .await
        .expect("停止节点");
}

/// chat_media_file 输出键名逐字对齐契约 §12.1（main 此前无直接断言）。
#[test]
fn chat_media_file_json_keys_match_contract() {
    let value = serde_json::to_value(ChatMediaFileJson {
        path: "asset://localhost/x".into(),
        mime: "image/png".into(),
        name: "x.png".into(),
    })
    .expect("序列化媒体文件");
    assert_eq!(
        value,
        json!({"path": "asset://localhost/x", "mime": "image/png", "name": "x.png"}),
        "chat_media_file 输出键名须逐字对齐契约"
    );
}

/// 双回环真节点经命令层成功投递（命令层 happy path 此前无覆盖）：
/// B 发邀请 → A 同意（双向互为好友）→ chat_send 2000 字符边界文本 →
/// delivered=true，replyTo 透传与对端命令层历史读回。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn command_layer_loopback_delivery_with_reply_to() {
    let dir = cmd_dir("loop");
    let _guard = DirGuard(dir.clone());
    let app_a = tauri::test::mock_app();
    let handle_a = app_a.handle().clone();
    handle_a.manage(AppState::new(dir.join("a")));
    let state_a = handle_a.state::<AppState>();
    let status_a = commands::node_start(
        handle_a.clone(),
        state_a.clone(),
        loopback_config(&dir.join("a")),
    )
    .await
    .expect("启动 A");
    let peer_a = status_a.peer_id.expect("A peer_id 必有");
    let app_b = tauri::test::mock_app();
    let handle_b = app_b.handle().clone();
    handle_b.manage(AppState::new(dir.join("b")));
    let state_b = handle_b.state::<AppState>();
    let status_b = commands::node_start(
        handle_b.clone(),
        state_b.clone(),
        loopback_config(&dir.join("b")),
    )
    .await
    .expect("启动 B");
    let peer_b = status_b.peer_id.expect("B peer_id 必有");

    chat_friend_invite(
    // 邀请流：B 发邀请（A 在线，实时送达）→ A 同意（双向互为好友）
    let invite = chat_friend_invite(

        state_b.clone(),
        peer_a.clone(),
        "A".into(),
        status_a.listen_addrs,
    )
    .await
    .expect("B 发邀请");
    assert!(invite.delivered, "A 在线必须实时送达");
    let accepted = chat_invite_accept(state_a.clone(), peer_b.clone(), "B".into())
        .await
        .expect("A 同意来邀");
    assert_eq!(accepted.peer_id, peer_b);
    let a_invites = chat_invites_list(state_a.clone()).await.expect("A 邀请列表");
    assert!(
        a_invites.iter().all(|i| i.peer_id != peer_b),
        "同意后来邀必须清除"
    );
    let report = chat_send(
        state_b.clone(),
        peer_a,
        ChatKind::Text,
        Some("x".repeat(2000)),
        None,
        Some("quoted-1".into()),
    )
    .await
    .expect("2000 字符边界必须通过校验并送达");
    assert!(report.delivered, "回环对端在线必须真实送达");
    assert_eq!(
        report.message.status,
        ChatStatus::Delivered,
        "实时送达后状态应 delivered"
    );
    assert_eq!(
        report.message.reply_to.as_deref(),
        Some("quoted-1"),
        "replyTo 透传"
    );

    let hist = chat_history(state_a.clone(), peer_b, None, None)
        .await
        .expect("A 侧命令层历史");
    assert_eq!(hist.len(), 1, "对端应落盘 1 条: {hist:?}");
    assert_eq!(
        hist[0].reply_to.as_deref(),
        Some("quoted-1"),
        "入站 replyTo 保留"
    );
    assert_eq!(
        hist[0].text.as_deref(),
        Some("x".repeat(2000).as_str()),
        "2000 字符完整"
    );

    commands::node_stop(handle_a.clone(), state_a.clone())
        .await
        .expect("停止 A");
    commands::node_stop(handle_b.clone(), state_b.clone())
        .await
        .expect("停止 B");
}
