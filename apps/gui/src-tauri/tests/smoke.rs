//! G-A2 双节点真实冒烟（不起 webview）：直接调命令层函数，验证
//! mDNS 互发现、A 对 B echo ping 拿 rtt、DialHop 事件流可观测。
//!
//! 临时目录统一 smoke_p2p_gui_ 前缀，测试结束即清理（scripts/gui-smoke.sh 兜底）。

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

use std::sync::{Arc, Mutex};

use p2p::NodeEvent;
use tauri::test::MockRuntime;
use tauri::{App, Listener, Manager, State};
use tokio::sync::broadcast;

use p2p_console::commands;
use p2p_console::state::AppState;
use p2p_console::types::{GuiConfig, NodeEventJson, NodeStatus};

/// mDNS 互发现窗口；同机双实例通常秒级，留足冷启动余量。
const DISCOVER_TIMEOUT: Duration = Duration::from_secs(30);
/// echo ping 预算（含拨号回退）。
const PING_TIMEOUT_MS: u64 = 15_000;

/// 空闲端口探测：临时 bind TCP 拿端口即释放（QUIC/TCP 共用近似探测）。
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("绑定临时端口")
        .local_addr()
        .expect("读取端口")
        .port()
}

/// 冒烟临时目录（smoke 前缀，测试结束由 guard 清理）。
fn smoke_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("smoke_p2p_gui_{tag}_{}", std::process::id()))
}

/// 退出清理：目录删不掉留告警，不 panic（避免掩盖真失败原因）。
struct DirGuard(PathBuf);

impl Drop for DirGuard {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("[smoke] 清理临时目录失败 {}: {e}", self.0.display());
            }
        }
    }
}

/// node-event JSON 捕获日志：经真实 emit 通道落盘，供运行时符合性断言。
type EventLog = Arc<Mutex<Vec<serde_json::Value>>>;

/// 在 mock handle 上挂 node-event 监听，捕获 emit 出口的真实 JSON 载荷。
fn capture_events(handle: &tauri::AppHandle<MockRuntime>) -> EventLog {
    let log: EventLog = Arc::default();
    let sink = log.clone();
    handle.listen_any("node-event", move |event| {
        match serde_json::from_str::<serde_json::Value>(event.payload()) {
            Ok(payload) => sink.lock().expect("事件日志锁中毒").push(payload),
            Err(e) => eprintln!("[smoke] node-event 载荷非法 JSON: {e}"),
        }
    });
    log
}

fn node_config(dir: &Path, quic: u16, tcp: u16) -> GuiConfig {
    GuiConfig {
        quic_port: quic,
        tcp_port: tcp,
        enable_mdns: true,
        data_dir: dir.to_string_lossy().into_owned(),
        bootstrap: Vec::new(),
        relay_addrs: Vec::new(),
        advertised_addrs: Vec::new(),
        observation_port: None,
        observation_addrs: Vec::new(),
    }
}

/// 起一个节点：独立 mock App + AppState，返回（App, 状态, 事件订阅）。
async fn start_node(
    tag: &str,
    quic: u16,
    tcp: u16,
) -> (
    App<MockRuntime>,
    NodeStatus,
    broadcast::Receiver<NodeEvent>,
    EventLog,
) {
    let dir = smoke_dir(tag);
    std::fs::create_dir_all(&dir).expect("创建冒烟数据目录");
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    handle.manage(AppState::new(dir.clone()));
    let log = capture_events(&handle);
    let state: State<'_, AppState> = handle.state();
    let status = commands::node_start(handle.clone(), state, node_config(&dir, quic, tcp))
        .await
        .unwrap_or_else(|e| panic!("节点 {tag} 启动失败: {e}"));
    let rx = handle
        .state::<AppState>()
        .subscribe_events()
        .await
        .expect("订阅节点事件");
    (app, status, rx, log)
}

/// 在超时内等第一条满足谓词的事件；超时返回 None，通道关闭立即返回 None。
async fn wait_event(
    rx: &mut broadcast::Receiver<NodeEvent>,
    timeout: Duration,
    pred: impl Fn(&NodeEvent) -> bool,
) -> Option<NodeEvent> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let budget = deadline.saturating_duration_since(tokio::time::Instant::now());
        if budget.is_zero() {
            return None;
        }
        match tokio::time::timeout(budget, rx.recv()).await {
            Ok(Ok(ev)) if pred(&ev) => return Some(ev),
            Ok(Ok(_)) => continue,
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(_)) => return None,
            Err(_) => return None,
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_nodes_discover_ping_and_observe_dialhop() {
    let _guard_a = DirGuard(smoke_dir("a"));
    let _guard_b = DirGuard(smoke_dir("b"));
    let _guard_cfg = DirGuard(smoke_dir("cfg"));

    // 0. 命令层配置往返（不依赖网络）：save 返回值与 get 读回逐字段一致
    {
        let app = tauri::test::mock_app();
        let handle = app.handle().clone();
        handle.manage(AppState::new(smoke_dir("cfg")));
        let state: State<'_, AppState> = handle.state();
        let cfg = node_config(&smoke_dir("cfg"), free_port(), free_port());
        let saved = commands::config_save(state.clone(), cfg.clone())
            .await
            .expect("保存配置");
        assert_eq!(saved, cfg, "config_save 返回值应与输入一致");
        let loaded = commands::config_get(state).await.expect("读取配置");
        assert_eq!(loaded, cfg, "config_get 应与 config_save 逐字段一致");
    }

    let (app_a, status_a, mut rx_a, log_a) = start_node("a", free_port(), free_port()).await;
    let (app_b, status_b, mut rx_b, log_b) = start_node("b", free_port(), free_port()).await;
    let peer_b = status_b
        .peer_id
        .clone()
        .expect("节点 B 启动后必有 PeerId");
    let peer_a = status_a.peer_id.clone().expect("节点 A 启动后必有 PeerId");

    // 1. mDNS 互发现：双方都收到对方 PeerDiscovered
    let seen_b = wait_event(&mut rx_a, DISCOVER_TIMEOUT, |ev| {
        matches!(ev, NodeEvent::PeerDiscovered { peer, .. } if peer.to_string() == peer_b)
    })
    .await;
    assert!(seen_b.is_some(), "A 未在 {DISCOVER_TIMEOUT:?} 内经 mDNS 发现 B");
    let seen_a = wait_event(&mut rx_b, DISCOVER_TIMEOUT, |ev| {
        matches!(ev, NodeEvent::PeerDiscovered { peer, .. } if peer.to_string() == peer_a)
    })
    .await;
    assert!(seen_a.is_some(), "B 未在 {DISCOVER_TIMEOUT:?} 内经 mDNS 发现 A");

    // 2. DialHop 事件流可观测：对确定不可达目标拨号，必得 direct 失败跳
    //    （随机 PeerId 保证不是 self-dial；127.0.0.1:1 毫秒级拒连）
    let ghost = bs58::encode([11u8; 32]).into_string();
    let report_bad = commands::peer_dial(
        app_a.handle().state::<AppState>(),
        format!("{ghost}@127.0.0.1/t1"),
    )
    .await
    .expect("拨号命令本身不应失败");
    assert!(!report_bad.ok, "不可达目标不应拨通");
    assert!(
        report_bad
            .hops
            .iter()
            .any(|h| matches!(h.hop, p2p_console::types::HopKind::Direct) && !h.ok),
        "DialReport 应含 direct 失败跳: {:?}",
        report_bad.hops
    );
    let hop_event = wait_event(&mut rx_a, Duration::from_secs(10), |ev| {
        matches!(ev, NodeEvent::DialHop { ok: false, .. })
    })
    .await;
    assert!(hop_event.is_some(), "事件流未观测到 DialHop 失败跳");

    // 3. 显式登记 B 地址并拨通（用 B 监听地址，node 层已把 0.0.0.0 换成 127.0.0.1）
    let b_addr = status_b
        .listen_addrs
        .first()
        .cloned()
        .expect("节点 B 必有监听地址");
    let report = commands::peer_dial(
        app_a.handle().state::<AppState>(),
        format!("{peer_b}@{b_addr}"),
    )
    .await
    .expect("对 B 拨号命令不应失败");
    assert!(report.ok, "对 B 拨号应成功: {:?}", report.hops);

    // 4. A 对 B echo ping 拿 rtt
    let outcome = commands::peer_ping(
        app_a.handle().state::<AppState>(),
        peer_b.clone(),
        PING_TIMEOUT_MS,
    )
    .await
    .expect("ping 命令不应失败");
    assert!(outcome.ok, "ping 应成功: {:?}", outcome.error);
    let rtt = outcome.rtt_ms.expect("成功 ping 必有 rtt");
    assert!(rtt <= PING_TIMEOUT_MS, "rtt {rtt}ms 超出预算");

    // 4.5 metrics_history：采样任务随启动即采首点，运行期返回非空且字段合规
    let history = loop {
        let history = commands::metrics_history(app_a.handle().state::<AppState>())
            .await
            .expect("metrics_history 命令不应失败");
        if !history.is_empty() {
            break history;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    assert!(history[0].t_ms > 0, "采样点应带毫秒戳");
    assert!(
        history.len() <= p2p_console::history::HISTORY_CAP,
        "序列不得超过环形上限"
    );

    // 5. tsMs 运行时符合性：emit 出口捕获的每条 JSON 均含数值型 tsMs（契约 §2 修订）
    for (name, log) in [("A", &log_a), ("B", &log_b)] {
        let events = log.lock().expect("事件日志锁中毒");
        assert!(!events.is_empty(), "{name} 侧应捕获到 node-event 载荷");
        for (i, payload) in events.iter().enumerate() {
            assert!(
                payload["tsMs"].is_u64(),
                "{name} 第 {i} 条事件 tsMs 非数值: {payload}"
            );
            serde_json::from_value::<NodeEventJson>(payload.clone()).unwrap_or_else(|e| {
                panic!("{name} 第 {i} 条载荷不合约: {e}: {payload}");
            });
        }
        assert!(
            events.iter().any(|v| v["type"] == "node_started"),
            "{name} 侧应捕获到自产 node_started（盖戳路径覆盖桥接层自产事件）"
        );
    }

    // 6. 幂等收尾
    let stopped = commands::node_stop(app_a.handle().clone(), app_a.handle().state::<AppState>())
        .await
        .expect("停止 A");
    assert!(!stopped.running, "A 停止后 running 应为 false");
    let stopped_b = commands::node_stop(app_b.handle().clone(), app_b.handle().state::<AppState>())
        .await
        .expect("停止 B");
    assert!(!stopped_b.running, "B 停止后 running 应为 false");
}
