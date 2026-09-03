//! E8 空闲回收与统一活跃度判定集成回归（验收文件名固定 conn_reclaim.rs）。
//! 覆盖：① 空闲回收+使用中豁免（消融锚点 usage.rs is_idle 的 in_flight
//! 判空，撤掉即 reclaim_exempts_in_flight_stream 必红）；② 关闭原因三档
//! （Idle/Error/Refused）；③ 多源信号单一判定（不重复翻转）。

use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};

use p2p_identity::{Keypair, PeerId};
use p2p_mux::BoxedStream;
use p2p_protocol::{
    open_with_protocol, read_frame, write_frame, HandlerRegistry, ProtocolHandler, ProtocolId,
};
use p2p_swarm::{
    gate_fn, AddrSource, CloseReason, ConnState, LifecycleEvent, LivenessSource, PeerLiveness,
    ReclaimConfig, Swarm, SwarmConfig,
};
use tokio::sync::broadcast;

const ECHO: &str = "/itest/echo/1";
const THRESHOLD: Duration = Duration::from_secs(1);
const SCAN: Duration = Duration::from_millis(200);
const IDLE_WAIT: Duration = Duration::from_millis(2500);
const WINDOW: Duration = Duration::from_secs(5);

struct Echo;

#[async_trait::async_trait]
impl ProtocolHandler for Echo {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(ECHO).expect("valid protocol id")
    }
    async fn handle(&self, mut stream: BoxedStream) -> io::Result<()> {
        let req = read_frame(&mut stream).await?;
        write_frame(&mut stream, &req).await
    }
}

fn config() -> SwarmConfig {
    SwarmConfig {
        keypair: Arc::new(Keypair::generate()),
        quic_port: 0,
        tcp_port: 0,
        registry: Arc::new(HandlerRegistry::default()),
        relay_addrs: Vec::new(),
        advertised_addrs: Vec::new(),
    }
}

/// 短阈值回收配置（默认值语义见 ReclaimConfig，此处仅压缩测试时长）。
fn fast_reclaim() -> ReclaimConfig {
    ReclaimConfig {
        enabled: true,
        idle_threshold: THRESHOLD,
        scan_interval: SCAN,
    }
}

/// a 挂快速回收，b 挂默认配置；两侧注册 echo，a 登记 b 的全部地址。
async fn pair() -> (Arc<Swarm>, Arc<Swarm>, PeerId) {
    let a = Swarm::start_with_reclaim(config(), Default::default(), fast_reclaim())
        .await
        .expect("bind a");
    let b = Swarm::start(config()).await.expect("bind b");
    a.register(Arc::new(Echo));
    b.register(Arc::new(Echo));
    let peer_b = b.local_peer_id();
    a.add_peer_addresses(peer_b, b.listen_addrs());
    (a, b, peer_b)
}

/// 收集事件直至 pred 命中，返回途中全部事件（含命中帧）。
async fn wait_for(
    rx: &mut broadcast::Receiver<LifecycleEvent>,
    what: &str,
    mut pred: impl FnMut(&LifecycleEvent) -> bool,
) -> Vec<LifecycleEvent> {
    let deadline = tokio::time::Instant::from_std(Instant::now() + WINDOW);
    let mut seen = Vec::new();
    loop {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Ok(ev)) => {
                let hit = pred(&ev);
                seen.push(ev);
                if hit {
                    return seen;
                }
            }
            other => panic!("event not observed within {WINDOW:?}: {what}; got {other:?}"),
        }
    }
}

/// ConnectionClosed 谓词工厂：指定 peer 与原因档位。
fn close_pred(peer: PeerId, reason: CloseReason) -> impl Fn(&LifecycleEvent) -> bool {
    move |e| {
        matches!(
            e,
            LifecycleEvent::ConnectionClosed { peer: p, reason: r } if *p == peer && *r == reason
        )
    }
}

fn has_close(events: &[LifecycleEvent], peer: PeerId, reason: CloseReason) -> bool {
    events.iter().any(close_pred(peer, reason))
}

/// 睡 wait 后清空通道存量（供「不得发生」类断言）。
async fn settle(
    rx: &mut broadcast::Receiver<LifecycleEvent>,
    wait: Duration,
) -> Vec<LifecycleEvent> {
    tokio::time::sleep(wait).await;
    let mut seen = Vec::new();
    while let Ok(e) = rx.try_recv() {
        seen.push(e);
    }
    seen
}

fn drain_liveness(rx: &mut broadcast::Receiver<LifecycleEvent>) -> Vec<PeerLiveness> {
    let mut out = Vec::new();
    while let Ok(e) = rx.try_recv() {
        if let LifecycleEvent::PeerLiveness(p) = e {
            out.push(p);
        }
    }
    out
}

async fn echo_round(a: &Swarm, peer: &PeerId) {
    let id = ProtocolId::new(ECHO).expect("id");
    let raw = a.open_stream(peer, &id).await.expect("open echo stream");
    // open_stream 交付裸流：协议 ID 首帧由调用方写（design §5.4 契约）
    let mut stream = open_with_protocol(raw, &id).await.expect("handshake");
    write_frame(&mut stream, b"ping").await.expect("write");
    let reply = read_frame(&mut stream).await.expect("read");
    assert_eq!(reply, b"ping", "echo round must return payload");
}

/// 验收 1a：空闲回收——闲置超阈值被回收、出池、出册不自动重连、可按需重拨。
#[tokio::test]
async fn idle_connection_reclaimed_without_auto_reconnect() {
    let (a, b, peer) = pair().await;
    let mut ev = a.subscribe_lifecycle();
    a.connect(peer).await.expect("dial b");
    echo_round(&a, &peer).await;
    wait_for(&mut ev, "idle close", close_pred(peer, CloseReason::Idle)).await;
    assert_eq!(
        a.metrics().active_connections,
        0,
        "reclaimed conn must leave pool"
    );
    // 回收不是故障：出册后不得自动重连；对端可达时按需重拨必须成功。
    let noise = settle(&mut ev, IDLE_WAIT).await;
    assert!(
        !noise.iter().any(|e| matches!(e,
            LifecycleEvent::PeerStateChanged { peer: p, to: ConnState::Connecting | ConnState::BackingOff, .. }
                if *p == peer)),
        "reclaim must not trigger auto reconnect: {noise:?}"
    );
    a.connect(peer)
        .await
        .expect("re-dial after reclaim must succeed");
    assert_eq!(a.metrics().active_connections, 1);
    a.shutdown();
    b.shutdown();
}

/// 验收 1b：使用中豁免——在途流连接绝不回收；流释放后下一轮扫描回收。
/// 消融：usage.rs is_idle 撤掉 in_flight 判空，本用例断言段必红。
#[tokio::test]
async fn reclaim_exempts_in_flight_stream() {
    let (a, b, peer) = pair().await;
    let mut ev = a.subscribe_lifecycle();
    a.connect(peer).await.expect("dial b");
    let id = ProtocolId::new(ECHO).expect("id");
    let held = a.open_stream(&peer, &id).await.expect("open held stream");
    // 越过阈值 + 数个扫描周期：有在途流即不得回收
    let noise = settle(&mut ev, THRESHOLD + SCAN * 5).await;
    assert_eq!(a.metrics().active_connections, 1, "in-flight must exempt");
    assert!(
        !has_close(&noise, peer, CloseReason::Idle),
        "in-flight: {noise:?}"
    );
    // 流释放：在途计数归还，下一轮扫描回收
    drop(held);
    wait_for(
        &mut ev,
        "idle close after release",
        close_pred(peer, CloseReason::Idle),
    )
    .await;
    assert_eq!(a.metrics().active_connections, 0);
    a.shutdown();
    b.shutdown();
}

/// 验收 2：关闭原因三档之 Error/Refused（Idle 见验收 1a/1b）。逐场景独立实例。
#[tokio::test]
async fn close_reason_error_and_refused_archived() {
    // Error：对端关停拆链归因 Error；用默认回收配置防快速回收抢先淹没该档。
    let a = Swarm::start(config()).await.expect("bind a");
    let b = Swarm::start(config()).await.expect("bind b");
    let peer = b.local_peer_id();
    a.add_peer_addresses(peer, b.listen_addrs());
    let mut ev = a.subscribe_lifecycle();
    a.connect(peer).await.expect("dial b");
    tokio::time::sleep(Duration::from_millis(300)).await;
    b.shutdown();
    let events = wait_for(&mut ev, "error close", close_pred(peer, CloseReason::Error)).await;
    assert!(has_close(&events, peer, CloseReason::Error), "{events:?}");
    a.shutdown();

    // Refused：监听方门禁拒收入站连接，归档在监听方
    let listener = Swarm::start(config()).await.expect("bind listener");
    listener.set_gate(Arc::new(gate_fn(|_| false)));
    let mut ev_l = listener.subscribe_lifecycle();
    let dialer = Swarm::start(config()).await.expect("bind dialer");
    let dialer_id = dialer.local_peer_id();
    dialer.add_peer_addresses(listener.local_peer_id(), listener.listen_addrs());
    dialer
        .connect(listener.local_peer_id())
        .await
        .expect("transport dial lands");
    wait_for(
        &mut ev_l,
        "refused",
        close_pred(dialer_id, CloseReason::Refused),
    )
    .await;
    listener.shutdown();
    dialer.shutdown();
}

/// 验收 3：多源信号单一判定——同一对端多源死信号只产出一条 PeerLiveness，
/// 重复死信号不翻面；真实连接建成恰好恢复翻转一条 alive=true。
#[tokio::test]
async fn multi_source_signals_judge_once() {
    let (a, b, peer) = pair().await;
    let mut ev = a.subscribe_lifecycle();
    // 双源活信号先记账（Unknown→Alive 静默，不与发现/连接事件重复播报）
    a.add_peer_addresses_with_source(peer, b.listen_addrs(), AddrSource::Mdns);
    a.add_peer_addresses_with_source(peer, b.listen_addrs(), AddrSource::Rendezvous);
    let pre = drain_liveness(&mut ev);
    assert!(pre.is_empty(), "first alive signal must be silent: {pre:?}");
    // 双源死信号：只产出一条判定
    a.on_peer_expired(peer);
    a.on_relay_slot_lost(peer);
    let judged = drain_liveness(&mut ev);
    assert_eq!(
        judged.len(),
        1,
        "multi-source dead must judge once: {judged:?}"
    );
    assert!(!judged[0].alive);
    assert_eq!(judged[0].source, LivenessSource::Discovery);
    let now = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(
        judged[0].last_seen_unix + 5 >= now,
        "last_seen must be seconds-fresh: {} vs {}",
        judged[0].last_seen_unix,
        now
    );
    // 重复死信号不再翻面
    a.on_peer_expired(peer);
    a.on_relay_slot_lost(peer);
    assert!(
        drain_liveness(&mut ev).is_empty(),
        "repeat dead must not re-flip"
    );
    // 恢复：真实连接建成 = Connection 活信号，恰好一条 alive=true
    a.connect(peer).await.expect("re-dial after dead judgment");
    let events = wait_for(
        &mut ev,
        "liveness recovery",
        |ev| matches!(ev, LifecycleEvent::PeerLiveness(p) if p.alive && p.peer == peer),
    )
    .await;
    let alive: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            LifecycleEvent::PeerLiveness(p) => Some(*p),
            _ => None,
        })
        .collect();
    assert_eq!(alive.len(), 1, "exactly one recovery flip: {events:?}");
    assert_eq!(alive[0].source, LivenessSource::Connection);
    a.shutdown();
    b.shutdown();
}
