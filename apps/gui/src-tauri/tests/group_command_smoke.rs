//! group_create/group_send 双回环真节点命令层冒烟（G4，T32/IM-T49 先例口径）。
//!
//! 命令真实可达非仅类型存在：A 建群（B 在册、带可拨地址）→ roster 推送落 B →
//! A group_send 真实送达并计入 ACK → B 命令层历史读回。端口 0 回环离线配置，
//! 不启动任何外部服务；serde 形状断言归 group_contract.rs，此处不重复。

use std::path::{Path, PathBuf};

use p2p_chat::{ChatKind, ChatStatus, GroupState};
use p2p_console::chat::{chat_friend_invite, chat_invite_accept, chat_invites_list};
use p2p_console::commands;
use p2p_console::group::{group_create, group_disband, group_history, group_list, group_send};
use p2p_console::state::AppState;
use p2p_console::types::GuiConfig;
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
    let dir = std::env::temp_dir().join(format!("group-cmd-smoke-{tag}-{}", std::process::id()));
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
                "[group-cmd-smoke] 清理临时目录失败 {}: {e}",
                self.0.display()
            );
        }
    }
}

/// 启动单节点，返回（app 句柄保持存活, AppHandle, PeerId, 监听地址）。
async fn start_node(
    dir: &Path,
) -> (
    tauri::App<tauri::test::MockRuntime>,
    tauri::AppHandle<tauri::test::MockRuntime>,
    String,
    Vec<String>,
) {
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    handle.manage(AppState::new(dir.to_path_buf()));
    let state = handle.state::<AppState>();
    let status = commands::node_start(handle.clone(), state.clone(), loopback_config(dir))
        .await
        .expect("启动回环节点");
    (
        app,
        handle,
        status.peer_id.expect("peer_id 必有"),
        status.listen_addrs,
    )
}

/// A 建群 → roster 推送 → A 群发文本 → B ACK → B 历史读回（全链经命令层）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn group_create_and_send_loopback_delivery() {
    let dir = cmd_dir("loop");
    let _guard = DirGuard(dir.clone());
    let (app_a, handle_a, peer_a, _) = start_node(&dir.join("a")).await;
    let (app_b, handle_b, peer_b, addrs_b) = start_node(&dir.join("b")).await;
    let state_a = handle_a.state::<AppState>();
    let state_b = handle_b.state::<AppState>();

    // 邀请流建双向好友：A 邀请 B（B 在线）→ B 同意（群成员 ⊆ 好友簿）
    let invite = chat_friend_invite(state_a.clone(), peer_b.clone(), "B".into(), addrs_b)
        .await
        .expect("A 发邀请");
    assert!(invite.delivered, "B 在线必须实时送达");
    chat_invite_accept(state_b.clone(), peer_a.clone(), "A".into())
        .await
        .expect("B 同意来邀");
    let invites_b = chat_invites_list(state_b.clone()).await.expect("B 邀请列表");
    assert!(invites_b.iter().all(|i| i.peer_id != peer_a));
    let group = group_create(state_a.clone(), "项目群".into(), vec![peer_b.clone()])
        .await
        .expect("建群");
    assert_eq!(group.owner, peer_a, "owner=建群人");
    assert_eq!(group.members, vec![peer_a.clone(), peer_b.clone()]);
    assert_eq!(group.rev, 0, "建群 rev=0");

    let roster_b = group_list(state_b.clone()).await.expect("B 读群列表");
    assert_eq!(roster_b.len(), 1, "roster 应已推送落 B: {roster_b:?}");
    assert_eq!(roster_b[0].group_id, group.group_id, "同群");
    assert_eq!(roster_b[0].owner, peer_a, "首个 roster 落定 owner");

    let report = group_send(
        state_a.clone(),
        group.group_id.clone(),
        ChatKind::Text,
        Some("大家好".into()),
        None,
        Some("q-1".into()),
    )
    .await
    .expect("群发文本");
    assert!(report.delivered, "在线成员必须真实送达: {report:?}");
    assert_eq!(report.acked, 1, "B 的 ACK 必须计入");
    assert_eq!(report.recipients, 1, "目标成员数 n-1");
    assert_eq!(report.message.sender_id, peer_a, "发端=作者");
    assert_eq!(report.message.status, ChatStatus::Delivered);
    assert_eq!(
        report.message.reply_to.as_deref(),
        Some("q-1"),
        "replyTo 透传"
    );

    let hist = group_history(state_b.clone(), group.group_id.clone(), None, None)
        .await
        .expect("B 命令层历史");
    assert_eq!(hist.len(), 1, "B 应落盘 1 条: {hist:?}");
    assert_eq!(hist[0].id, report.message.id, "同一消息 id 端到端一致");
    assert_eq!(hist[0].sender_id, peer_a, "B 视角作者=A");
    assert_eq!(hist[0].text.as_deref(), Some("大家好"));
    assert_eq!(
        hist[0].reply_to.as_deref(),
        Some("q-1"),
        "入站 replyTo 保留"
    );
    assert!(hist[0].acks.is_empty(), "收到的消息 acks 恒空（§4）");

    commands::node_stop(handle_a.clone(), state_a.clone())
        .await
        .expect("停止 A");
    commands::node_stop(handle_b.clone(), state_b.clone())
        .await
        .expect("停止 B");
    let _ = (app_a, app_b);
}

/// 解散回环（G6）：A 命令层解散 → 返回 state=disbanded 且 rev 推进 → B 经
/// G_KICK(disbanded) 命令层列表置位；重复解散显式 Err（owner/active 守卫）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn group_disband_loopback_member_receipt() {
    let dir = cmd_dir("disband");
    let _guard = DirGuard(dir.clone());
    let (app_a, handle_a, peer_a, _) = start_node(&dir.join("a")).await;
    let (app_b, handle_b, peer_b, addrs_b) = start_node(&dir.join("b")).await;
    let state_a = handle_a.state::<AppState>();
    let state_b = handle_b.state::<AppState>();

    // 邀请流建双向好友（群成员必须 ⊆ 好友簿）
    let invite = chat_friend_invite(state_a.clone(), peer_b.clone(), "B".into(), addrs_b)
        .await
        .expect("A 发邀请");
    assert!(invite.delivered, "B 在线必须实时送达");
    chat_invite_accept(state_b.clone(), peer_a.clone(), "A".into())
        .await
        .expect("B 同意来邀");
    let group = group_create(state_a.clone(), "解散群".into(), vec![peer_b.clone()])
        .await
        .expect("建群");

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    let roster_b = loop {
        let list = group_list(state_b.clone()).await.expect("B 读群列表");
        if !list.is_empty() {
            break list;
        }
        assert!(tokio::time::Instant::now() < deadline, "roster 未到 B");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    };
    assert_eq!(roster_b[0].state, GroupState::Active, "B 初始 active");

    let disbanded = group_disband(state_a.clone(), group.group_id.clone())
        .await
        .expect("解散");
    assert_eq!(disbanded.owner, peer_a, "owner=发起人");
    assert_eq!(disbanded.state, GroupState::Disbanded, "本端置 disbanded");
    assert_eq!(disbanded.rev, 1, "解散 rev 推进");

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let list = group_list(state_b.clone()).await.expect("B 读群列表");
        if list[0].state == GroupState::Disbanded {
            assert_eq!(list[0].group_id, group.group_id, "同群");
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "G_KICK 未达 B: {list:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let err = group_disband(state_a.clone(), group.group_id.clone())
        .await
        .expect_err("重复解散必须拒绝");
    assert!(err.contains("不可解散"), "err: {err}");

    commands::node_stop(handle_a.clone(), state_a.clone())
        .await
        .expect("停止 A");
    commands::node_stop(handle_b.clone(), state_b.clone())
        .await
        .expect("停止 B");
    let _ = (app_a, app_b);
}
