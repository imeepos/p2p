//! ginvite 四命令命令层回环（IMC2）：发起/列表/同意（roster 收敛）/拒绝与错误
//! 映射；事件冻结形状断言。装配口径同 group_command_smoke：mock runtime 直调
//! 命令层，端口 0 回环离线配置，不启动任何外部服务。

use std::path::{Path, PathBuf};
use std::time::Duration;

use p2p_chat::{GroupInviteDirection, GroupInviteState};
use p2p_console::chat::{chat_friend_invite, chat_invite_accept};
use p2p_console::ginvite::{
    chat_group_invite_accept, chat_group_invite_reject, chat_group_invite_send,
    chat_group_invites_list,
};
use p2p_console::group::{group_create, group_list};
use p2p_console::state::AppState;
use p2p_console::types::{GuiConfig, NodeEventJson};
use tauri::Manager;

/// 回环离线配置（T44：端口 0 内核动态分配，回环占位阻断云端端点回退）。
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
        authz_default_role: "friend".into(),
    }
}

fn cmd_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ginvite-cmd-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

/// 退出清理：删不掉留告警不 panic（避免掩盖真失败原因）。
struct DirGuard(PathBuf);

impl Drop for DirGuard {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            eprintln!("[ginvite-cmd] 清理临时目录失败 {}: {e}", self.0.display());
        }
    }
}

type TestApp = (
    tauri::App<tauri::test::MockRuntime>,
    tauri::AppHandle<tauri::test::MockRuntime>,
);

/// 启动单节点，返回（app 与句柄、PeerId、监听地址）；State 由用例体自取。
async fn start_node(dir: &Path) -> (TestApp, String, Vec<String>) {
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    handle.manage(AppState::new(dir.to_path_buf()));
    let state = handle.state::<AppState>();
    let status =
        p2p_console::commands::node_start(handle.clone(), state.clone(), loopback_config(dir))
            .await
            .expect("启动回环节点");
    let app = (app, handle);
    (
        app,
        status.peer_id.expect("peer_id 必有"),
        status.listen_addrs,
    )
}

/// 邀请流建双向好友（群成员 ⊆ 好友簿前置）；B 的监听地址已被 A 登记。
async fn make_friends(
    state_a: tauri::State<'_, AppState>,
    state_b: tauri::State<'_, AppState>,
    peer_a: &str,
    peer_b: &str,
    addrs_b: Vec<String>,
) {
    let report = chat_friend_invite(state_a.clone(), peer_b.to_string(), "B".into(), addrs_b)
        .await
        .expect("A 发好友邀请");
    assert!(report.delivered, "B 在线必须实时送达");
    chat_invite_accept(state_b.clone(), peer_a.to_string(), "A".into())
        .await
        .expect("B 同意来邀");
}

/// 发起入群邀请（B 在线应答 ACK，出参 delivered 即时为真；离线语义归 crate 测试）。
async fn send_invite(
    state: tauri::State<'_, AppState>,
    group_id: &str,
    peer_id: &str,
    note: Option<String>,
) -> p2p_chat::GroupInvite {
    chat_group_invite_send(state, group_id.to_string(), peer_id.to_string(), note)
        .await
        .expect("发起入群邀请")
}

/// 轮询取受邀者本地的 in 向条目（两侧账本各自 id，按 groupId+方向定位）。
async fn wait_in_entry(state: tauri::State<'_, AppState>, group_id: &str) -> p2p_chat::GroupInvite {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let list = chat_group_invites_list(state.clone())
            .await
            .expect("邀请列表");
        if let Some(entry) = list
            .iter()
            .find(|i| i.direction == GroupInviteDirection::In && i.group_id == group_id)
        {
            return entry.clone();
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "受邀者未见 in 向邀请: {list:?}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// 轮询直至条目收敛到目标态（deadline 内不至即断言失败并携带现值）。
async fn wait_state(state: tauri::State<'_, AppState>, invite_id: &str, want: GroupInviteState) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let list = chat_group_invites_list(state.clone())
            .await
            .expect("邀请列表");
        if list.iter().any(|i| i.id == invite_id && i.state == want) {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "邀请 {invite_id} 未收敛到 {want:?}: {list:?}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// 全链回环：A 发起（out/pending/delivered + note 透传）→ B 列表见 in/pending →
/// B 同意 → 两侧收敛 accepted → B 进群 roster。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ginvite_send_list_accept_loopback() {
    let dir = cmd_dir("accept");
    let _guard = DirGuard(dir.clone());
    let ((app_a, handle_a), peer_a, _) = start_node(&dir.join("a")).await;
    let ((app_b, handle_b), peer_b, addrs_b) = start_node(&dir.join("b")).await;
    let state_a = handle_a.state::<AppState>();
    let state_b = handle_b.state::<AppState>();
    make_friends(state_a.clone(), state_b.clone(), &peer_a, &peer_b, addrs_b).await;
    let group = group_create(state_a.clone(), "项目群".into(), vec![])
        .await
        .expect("建群");

    let entry = send_invite(
        state_a.clone(),
        &group.group_id,
        &peer_b,
        Some("一起干活".into()),
    )
    .await;
    assert!(entry.delivered, "B 在线必须实时送达");
    assert_eq!(entry.direction, GroupInviteDirection::Out);
    assert_eq!(entry.state, GroupInviteState::Pending);
    assert_eq!(entry.invitee, peer_b, "受邀人透传");
    assert_eq!(entry.inviter, peer_a, "邀请人=本机");
    assert_eq!(entry.note.as_deref(), Some("一起干活"), "note 透传");

    let in_entry = wait_in_entry(state_b.clone(), &group.group_id).await;
    assert_eq!(in_entry.state, GroupInviteState::Pending);
    assert_eq!(in_entry.group_name, "项目群", "群名随条目下发");
    assert_eq!(in_entry.invitee, peer_b, "受邀人=本机");
    assert_ne!(in_entry.id, entry.id, "两侧账本各自 id（设计如此）");

    chat_group_invite_accept(state_b.clone(), in_entry.id.clone())
        .await
        .expect("B 同意入群");
    wait_state(state_b.clone(), &in_entry.id, GroupInviteState::Accepted).await;
    wait_state(state_a.clone(), &entry.id, GroupInviteState::Accepted).await;
    let roster_b = group_list(state_b.clone()).await.expect("B 群列表");
    let g = roster_b
        .iter()
        .find(|g| g.group_id == group.group_id)
        .expect("roster 应收敛到 B");
    assert!(g.members.contains(&peer_b), "B 在列: {g:?}");

    let _ = (app_a, app_b);
}

/// 拒绝路径与错误映射：未知 id/非 owner 发起/非好友发起显式 Err；拒绝后两侧
/// 收敛 rejected，再同意被守卫拒绝。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ginvite_reject_path_and_error_mapping() {
    let dir = cmd_dir("reject");
    let _guard = DirGuard(dir.clone());
    let ((app_a, handle_a), peer_a, _) = start_node(&dir.join("a")).await;
    let ((app_b, handle_b), peer_b, addrs_b) = start_node(&dir.join("b")).await;
    let state_a = handle_a.state::<AppState>();
    let state_b = handle_b.state::<AppState>();
    make_friends(state_a.clone(), state_b.clone(), &peer_a, &peer_b, addrs_b).await;
    let group = group_create(state_a.clone(), "备选群".into(), vec![])
        .await
        .expect("建群");

    let err = chat_group_invite_accept(state_b.clone(), "no-such-id".into())
        .await
        .expect_err("未知邀请必须拒绝");
    assert!(err.contains("无待处理邀请"), "err: {err}");

    let err = chat_group_invite_send(
        state_b.clone(),
        group.group_id.clone(),
        peer_a.clone(),
        None,
    )
    .await
    .expect_err("非 owner 发起必须拒绝");
    // B 非成员，本地无该群：owned_group 先报群不存在（owner 守卫文案归 crate 测试）
    assert!(err.contains("群不存在"), "err: {err}");

    // 合法格式但不在好友簿的 peer（base58 32B，格式校验先于好友簿守卫）
    let stranger = "HCjw5d6mzG5Z9iGTebhRSHBZKjA1WuunTXkZN9gzmfWj";
    let err = chat_group_invite_send(
        state_a.clone(),
        group.group_id.clone(),
        stranger.into(),
        None,
    )
    .await
    .expect_err("非好友发起必须拒绝");
    assert!(err.contains("好友簿"), "err: {err}");

    let entry = send_invite(state_a.clone(), &group.group_id, &peer_b, None).await;
    assert!(entry.delivered, "B 在线必须实时送达");
    let in_entry = wait_in_entry(state_b.clone(), &group.group_id).await;
    chat_group_invite_reject(state_b.clone(), in_entry.id.clone(), Some("没空".into()))
        .await
        .expect("B 拒绝入群");
    wait_state(state_b.clone(), &in_entry.id, GroupInviteState::Rejected).await;
    wait_state(state_a.clone(), &entry.id, GroupInviteState::Rejected).await;

    let err = chat_group_invite_accept(state_b.clone(), in_entry.id.clone())
        .await
        .expect_err("拒绝后同意必须拒绝");
    assert!(err.contains("不可同意"), "err: {err}");

    let _ = (app_a, app_b);
}

/// 事件冻结形状：NodeEventJson::ChatGroupInvite 序列化 tag=chat_group_invite、
/// 载荷为 camelCase 邀请条目（透传通道归 node_event.rs，此处只冻结契约形状）。
#[test]
fn chat_group_invite_event_json_shape_is_frozen() {
    let invite = p2p_chat::GroupInvite {
        id: "i1".into(),
        group_id: "g1".into(),
        group_name: "群".into(),
        owner: "o".into(),
        inviter: "o".into(),
        invitee: "e".into(),
        note: None,
        direction: GroupInviteDirection::In,
        state: GroupInviteState::Pending,
        ts_ms: 5,
        delivered: false,
    };
    let v = serde_json::to_value(NodeEventJson::ChatGroupInvite {
        invite,
        ts_ms: None,
    })
    .expect("serialize");
    assert_eq!(v["type"], "chat_group_invite", "事件 tag 逐字冻结");
    assert_eq!(v["invite"]["groupId"], "g1");
    assert_eq!(v["invite"]["direction"], "in");
    assert_eq!(v["invite"]["state"], "pending");
}
