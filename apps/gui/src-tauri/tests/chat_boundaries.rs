//! Tauri chat 命令与 asset 输出边界（T36）：命令参数 serde 形状（缺失/null/camelCase/
//! 未知字段/全字段 roundtrip）、命令层校验错误映射与边界、asset URL 编码与绝对路径泄露。
//! 命令层用 mock runtime + 回环离线节点（bootstrap/relay/observation 全回环占位，阻断出厂云端端点回退），不启动外部服务。

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use p2p_chat::{
    validate_media, ChatEnvelope, ChatKind, ChatMediaMeta, ChatSendReport, ChatStatus, Sender,
    MAX_MESSAGE_SIZE,
};
use p2p_console::chat::{
    chat_friend_add, chat_friends_list, chat_history, chat_send, ChatMediaFileJson,
    ChatMediaInputJson,
};
use p2p_console::commands;
use p2p_console::state::AppState;
use p2p_console::types::GuiConfig;
use p2p_console::util::{to_asset_media, to_asset_url};
use serde_json::json;
use tauri::Manager;

fn cmd_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("chat-bound-cmd-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

/// 回环离线配置：非空回环占位阻断 build_node 的出厂云端端点回退（不启动外部服务）。
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

struct DirGuard(PathBuf);

impl Drop for DirGuard {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            eprintln!("[chat-boundaries] 清理临时目录失败 {}: {e}", self.0.display());
        }
    }
}

fn full_envelope() -> ChatEnvelope {
    ChatEnvelope {
        id: "m-1".into(),
        peer: "peer-a".into(),
        sender: Sender::Me,
        kind: ChatKind::Image,
        ts_ms: 1_720_000_000_000,
        text: None,
        media: Some(ChatMediaMeta {
            name: "my file 中文名.png".into(),
            mime: "image/png".into(),
            size: 3,
            path: Some("/data/chat/media/peer-a/m-1_my file 中文名.png".into()),
        }),
        status: ChatStatus::Pending,
    }
}

#[test]
fn command_input_json_rejects_missing_and_snake_case_tolerates_unknown() {
    let ok = json!({"name": "a.png", "mime": "image/png", "dataBase64": "eA=="});
    let input: ChatMediaInputJson = serde_json::from_value(ok).expect("camelCase 基础形状必须可解析");
    assert_eq!(input.data_base64, "eA==", "dataBase64 必须 camelCase 映射");
    let unknown = json!({"name": "a", "mime": "m", "dataBase64": "eA==", "futureField": 1});
    assert!(serde_json::from_value::<ChatMediaInputJson>(unknown).is_ok(), "未知字段应兼容忽略");
    let missing = json!({"name": "a.png", "mime": "image/png"});
    assert!(
        serde_json::from_value::<ChatMediaInputJson>(missing).is_err(),
        "缺 dataBase64 必填必须报错"
    );
    let snake = json!({"name": "a", "mime": "m", "data_base64": "eA=="});
    assert!(
        serde_json::from_value::<ChatMediaInputJson>(snake).is_err(),
        "snake_case 不得替代 camelCase"
    );
    let nulls = json!({
        "id": "m", "peer": "p", "sender": "them", "kind": "file", "tsMs": 0,
        "text": null, "media": null, "status": "failed", "unknownField": true
    });
    let parsed: ChatEnvelope = serde_json::from_value(nulls).expect("null Option 与未知字段应兼容");
    assert_eq!(parsed.text, None, "text null 必须映射 None");
    assert_eq!(parsed.media, None, "media null 必须映射 None");
    assert!(
        serde_json::from_value::<ChatEnvelope>(json!({"id": "m"})).is_err(),
        "缺必填字段必须报错"
    );
    let snake_env = json!({
        "id": "m", "peer": "p", "sender": "me", "kind": "text", "ts_ms": 1,
        "text": "x", "media": null, "status": "pending"
    });
    assert!(
        serde_json::from_value::<ChatEnvelope>(snake_env).is_err(),
        "ts_ms 不得替代 tsMs（camelCase 逐字对齐）"
    );
}

#[test]
fn chat_output_json_full_field_roundtrip() {
    let env = full_envelope();
    let env_value = serde_json::to_value(&env).expect("序列化消息");
    assert_eq!(env_value["tsMs"], json!(1_720_000_000_000_i64), "tsMs 必须 camelCase");
    assert_eq!(
        env_value["media"],
        json!({
            "name": "my file 中文名.png", "mime": "image/png", "size": 3,
            "path": "/data/chat/media/peer-a/m-1_my file 中文名.png"
        }),
        "media 全字段输出须逐字对齐契约"
    );
    assert_eq!(
        serde_json::from_value::<ChatEnvelope>(env_value).expect("roundtrip"),
        env,
        "ChatMessageJson 全字段 roundtrip 必须保真"
    );
    let report = ChatSendReport { message: full_envelope(), delivered: true };
    assert_eq!(
        serde_json::from_value::<ChatSendReport>(serde_json::to_value(&report).expect("序列化报告"))
            .expect("roundtrip"),
        report,
        "ChatSendReport 全字段 roundtrip 必须保真"
    );
    let file = ChatMediaFileJson {
        path: "asset://localhost/x".into(),
        mime: "image/png".into(),
        name: "x.png".into(),
    };
    assert_eq!(
        serde_json::to_value(&file).expect("序列化媒体文件"),
        json!({"path": "asset://localhost/x", "mime": "image/png", "name": "x.png"}),
        "chat_media_file 输出键名须逐字对齐契约"
    );
}

#[test]
fn asset_url_encodes_boundaries_and_never_leaks_raw_path() {
    let raw = "/data/chat/media/peer-a/m-1_my file 中文名.png";
    let url = to_asset_url(raw);
    let expected_prefix = if cfg!(target_os = "windows") {
        "http://asset.localhost/"
    } else {
        "asset://localhost/"
    };
    assert!(url.starts_with(expected_prefix), "平台前缀: {url}");
    let encoded = &url[expected_prefix.len()..];
    assert!(!encoded.contains('/'), "编码段不得残留裸分隔符: {encoded}");
    assert!(encoded.contains("%2F"), "路径分隔符必须编码: {encoded}");
    assert!(encoded.contains("%20"), "空格必须编码: {encoded}");
    assert!(encoded.contains("%E4%B8%AD"), "Unicode 须按 UTF-8 百分号编码: {encoded}");
    // 事件输出经 to_asset_media 后必须是 asset URL，禁止裸绝对路径泄露到前端
    let out = to_asset_media(full_envelope());
    let path = out.media.as_ref().and_then(|m| m.path.as_deref()).expect("媒体 path 应转换");
    assert!(path.starts_with(expected_prefix), "媒体 path 须为 asset URL: {path}");
    assert!(!path.contains("/data/chat"), "不得泄露裸绝对路径: {path}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn friend_commands_map_boundary_errors() {
    let dir = cmd_dir("friend");
    let _guard = DirGuard(dir.clone());
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    handle.manage(AppState::new(dir.clone()));
    let state = handle.state::<AppState>();
    let offline = chat_friends_list(state.clone()).await.expect_err("节点未启动必须 Err");
    assert!(offline.contains("节点未运行"), "实际: {offline}");
    let status = commands::node_start(handle.clone(), state.clone(), loopback_config(&dir))
        .await
        .expect("启动回环离线节点");
    assert!(status.running, "节点必须运行");
    let own = status.peer_id.expect("peer_id 必有");
    let remote = bs58::encode([42u8; 32]).into_string();
    let nick65 = "x".repeat(65);
    let cases = [
        ("!!!not-base58!!!".to_string(), "n".to_string(), "非法 peerId", "base58"),
        (own.clone(), "n".to_string(), "自加本机", "自己"),
        (remote.clone(), nick65, "昵称 65", "昵称"),
    ];
    for (peer, nickname, what, expect) in cases {
        let err = chat_friend_add(state.clone(), peer, nickname, vec![])
            .await
            .expect_err(what);
        assert!(err.contains(expect), "{what}: 实际 {err}");
    }
    let nick64 = chat_friend_add(state.clone(), remote.clone(), "x".repeat(64), vec![])
        .await
        .expect("昵称 64 恰好合法");
    assert_eq!(nick64.nickname.len(), 64, "64 字符边界必须通过");
    let bad_addr = chat_friend_add(state.clone(), remote.clone(), "n".into(), vec!["no-slash/u1".into()])
        .await
        .expect_err("非法地址");
    assert!(bad_addr.contains("地址"), "实际: {bad_addr}");
    let hist = chat_history(state.clone(), "!!!".into(), None, None)
        .await
        .expect_err("非法 peer 历史");
    assert!(hist.contains("base58"), "实际: {hist}");
    let empty = chat_history(state.clone(), remote.clone(), None, None)
        .await
        .expect("beforeId/limit 参数 null 须兼容");
    assert!(empty.is_empty(), "无消息历史应为空");
    commands::node_stop(handle.clone(), state.clone()).await.expect("停止节点");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_command_maps_boundary_errors() {
    let dir = cmd_dir("send");
    let _guard = DirGuard(dir.clone());
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    handle.manage(AppState::new(dir.clone()));
    let state = handle.state::<AppState>();
    commands::node_start(handle.clone(), state.clone(), loopback_config(&dir))
        .await
        .expect("启动回环离线节点");
    let status = commands::node_status(state.clone()).await.expect("状态快照");
    let own = status.peer_id.expect("peer_id 必有");
    let remote = bs58::encode([42u8; 32]).into_string();
    let b64 = |data: &[u8]| base64::engine::general_purpose::STANDARD.encode(data);
    let media = |mime: &str, data: &[u8]| ChatMediaInputJson {
        name: "a.bin".into(),
        mime: mime.into(),
        data_base64: b64(data),
    };
    let bad_b64 = ChatMediaInputJson {
        name: "a.bin".into(),
        mime: "application/octet-stream".into(),
        data_base64: "!!!not-base64!!!".into(),
    };
    // 参数 null/缺失语义：text=None、media=None 按空处理并给可读 Err
    type SendErrCase = (Option<String>, String, ChatKind, Option<ChatMediaInputJson>, &'static str, &'static str);
    let cases: Vec<SendErrCase> = vec![
        (None, remote.clone(), ChatKind::Text, None, "text 参数 null", "文本为空"),
        (None, remote.clone(), ChatKind::Image, None, "image 缺 media 参数", "必须携带附件"),
        (Some("hi".into()), own, ChatKind::Text, None, "自发自收", "自己"),
        (Some("   ".into()), remote.clone(), ChatKind::Text, None, "空白文本", "文本为空"),
        (Some("x".repeat(2001)), remote.clone(), ChatKind::Text, None, "2001 文本", "上限"),
        (None, remote.clone(), ChatKind::File, Some(media("application/octet-stream", b"")), "零字节媒体", "为空"),
        (None, remote.clone(), ChatKind::Image, Some(media("text/plain", b"a")), "MIME 不匹配", "MIME"),
        (None, remote, ChatKind::File, Some(bad_b64), "base64 非法", "base64"),
    ];
    for (text, peer, kind, input, what, expect) in cases {
        let err = chat_send(state.clone(), peer, kind, text, input)
            .await
            .expect_err(what);
        assert!(err.contains(expect), "{what}: 实际 {err}");
    }
    commands::node_stop(handle.clone(), state.clone()).await.expect("停止节点");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_boundary_acceptance_and_media_file_errors() {
    let dir = cmd_dir("accept");
    let _guard = DirGuard(dir.clone());
    // 双回环节点：B 对 A 投递，2000 字符边界文本真实送达（delivered 可观测）
    let app_a = tauri::test::mock_app();
    let handle_a = app_a.handle().clone();
    handle_a.manage(AppState::new(dir.join("a")));
    let state_a = handle_a.state::<AppState>();
    commands::node_start(handle_a.clone(), state_a.clone(), loopback_config(&dir.join("a"))).await.expect("启动 A");
    let status_a = commands::node_status(state_a.clone()).await.expect("A 状态");
    let peer_a = status_a.peer_id.expect("A peer_id 必有");
    let app_b = tauri::test::mock_app();
    let handle_b = app_b.handle().clone();
    handle_b.manage(AppState::new(dir.join("b")));
    let state_b = handle_b.state::<AppState>();
    commands::node_start(handle_b.clone(), state_b.clone(), loopback_config(&dir.join("b"))).await.expect("启动 B");
    chat_friend_add(state_b.clone(), peer_a.clone(), "A".into(), status_a.listen_addrs)
        .await
        .expect("B 登记 A");
    let send2000 = chat_send(state_b.clone(), peer_a, ChatKind::Text, Some("x".repeat(2000)), None);
    let ok2000 = tokio::time::timeout(Duration::from_secs(30), send2000)
        .await
        .expect("2000 文本应在期限内返回")
        .expect("2000 字符边界必须通过校验并送达");
    assert!(ok2000.delivered, "回环对端在线必须真实送达");
    assert!(
        validate_media(&ChatKind::Image, "image/png", MAX_MESSAGE_SIZE).is_ok(),
        "恰好 64MiB 必须通过"
    );
    assert!(
        validate_media(&ChatKind::Image, "image/png", MAX_MESSAGE_SIZE + 1).is_err(),
        "超 64MiB 必须拒绝"
    );
    commands::node_stop(handle_a.clone(), state_a.clone()).await.expect("停止 A");
    commands::node_stop(handle_b.clone(), state_b.clone()).await.expect("停止 B");
}
