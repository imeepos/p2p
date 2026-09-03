//! 对端连接生命周期验收（E6-S2，机械验收依赖本文件）：
//! 1. 状态转移可观测：首拨 Disconnected→Connecting→Connected；断链 Connected→BackingOff；
//!    退避到期 BackingOff→Connecting、重连失败 Connecting→BackingOff。
//! 2. 探活判离线：对端失聪（收 ping 不回）连续未命中触发 PeerDown，自动重连恢复后发 PeerUp。
//! 3. 退避复位语义（E5「重连退避复位」落地）：健康会话后的重连成功把退避归零——
//!    下次断链排定退避 == base；消融（注释 mark_connected 的复位块）本用例必红（4×base）。
//!
//! 断言全部基于事件流与状态查询（peer_state/peer_scheduled_backoff），不做墙钟测量。
//! 地址只登记 TCP：对已死端口的 TCP connect 立即 refused，重连失败快速且确定。

use std::io;
use std::sync::Arc;
use std::time::Duration;

use p2p_identity::{Keypair, PeerId};
use p2p_mux::BoxedStream;
use p2p_protocol::{read_frame, HandlerRegistry, ProtocolHandler, ProtocolId};
use p2p_swarm::{
    ConnState, LifecycleEvent, PeerLifecycleConfig, Swarm, SwarmConfig, PING_PROTOCOL,
};
use p2p_transport::TransportAddr;
use tokio::sync::broadcast;

const WINDOW: Duration = Duration::from_secs(5);
const BASE: Duration = Duration::from_millis(100);
/// 健康会话最短存活（测试值）：S1 持有 400ms 判健康，S2 闪断不参与复位判定。
const RESET_MIN_UPTIME: Duration = Duration::from_millis(300);

/// 失聪对端：收 ping 帧后不回直接关流——连接仍可建（mux 活着），ping 永远无响应。
struct Deaf;

#[async_trait::async_trait]
impl ProtocolHandler for Deaf {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(PING_PROTOCOL).expect("valid ping id")
    }
    async fn handle(&self, mut stream: BoxedStream) -> io::Result<()> {
        let _ = read_frame(&mut stream).await?;
        Ok(())
    }
}

fn lifecycle() -> PeerLifecycleConfig {
    PeerLifecycleConfig {
        probe_interval: Duration::from_millis(150),
        probe_timeout: Duration::from_secs(1),
        max_probe_misses: 2,
        reconnect_base: BASE,
        reconnect_max: Duration::from_secs(2),
        reconnect_jitter: 0.0,
        reset_min_uptime: RESET_MIN_UPTIME,
        ..PeerLifecycleConfig::default()
    }
}

/// 关闭探活（探活行为由 probe_misses_judge_peer_down 单独覆盖，这里隔离变量）。
fn lifecycle_without_probe() -> PeerLifecycleConfig {
    PeerLifecycleConfig {
        probe_interval: Duration::from_secs(3600),
        ..lifecycle()
    }
}

fn config(keypair: Keypair, registry: HandlerRegistry) -> SwarmConfig {
    SwarmConfig {
        keypair: Arc::new(keypair),
        quic_port: 0,
        tcp_port: 0,
        registry: Arc::new(registry),
        relay_addrs: Vec::new(),
        advertised_addrs: Vec::new(),
    }
}

fn tcp_addrs(swarm: &Swarm) -> Vec<TransportAddr> {
    swarm
        .listen_addrs()
        .into_iter()
        .filter(|addr| matches!(addr, TransportAddr::Tcp { .. }))
        .collect()
}

/// 累积事件直至谓词命中（命中事件也收入），超时即 panic 并附已收集事件。
async fn wait_for(
    rx: &mut broadcast::Receiver<LifecycleEvent>,
    what: &str,
    mut pred: impl FnMut(&LifecycleEvent) -> bool,
) -> Vec<LifecycleEvent> {
    let deadline = tokio::time::Instant::now() + WINDOW;
    let mut out = Vec::new();
    loop {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Ok(ev)) => {
                let hit = pred(&ev);
                out.push(ev);
                if hit {
                    return out;
                }
            }
            _ => panic!("event not observed within {WINDOW:?}: {what}; collected: {out:?}"),
        }
    }
}

fn saw_state(events: &[LifecycleEvent], peer: PeerId, from: ConnState, to: ConnState) -> bool {
    events.iter().any(|ev| {
        matches!(
            ev,
            LifecycleEvent::PeerStateChanged { peer: p, from: f, to: t }
            if *p == peer && *f == from && *t == to
        )
    })
}

async fn wait_connected(
    ev: &mut broadcast::Receiver<LifecycleEvent>,
    peer: PeerId,
    what: &str,
) -> Vec<LifecycleEvent> {
    wait_for(ev, what, |event| {
        matches!(event, LifecycleEvent::PeerStateChanged { peer: p, to: ConnState::Connected, .. } if *p == peer)
    })
    .await
}

/// 验收 1：首拨建档 Disconnected→Connecting→Connected；断链 Connected→BackingOff，
/// 退避到期 BackingOff→Connecting，重连失败 Connecting→BackingOff
/// （验收要求 Connecting→Connected 与任一态→BackingOff 双覆盖）。
#[tokio::test]
async fn state_transitions_and_backoff_on_link_loss() {
    let a = Swarm::start_with_lifecycle(
        config(Keypair::generate(), HandlerRegistry::default()),
        lifecycle_without_probe(),
    )
    .await
    .expect("bind a");
    let b = Swarm::start_with_lifecycle(
        config(Keypair::generate(), HandlerRegistry::default()),
        lifecycle_without_probe(),
    )
    .await
    .expect("bind b");
    let peer = b.local_peer_id();
    a.add_peer_addresses(peer, tcp_addrs(&b));
    let mut ev = a.subscribe_lifecycle();
    a.connect(peer).await.expect("dial b");
    let events = wait_connected(&mut ev, peer, "first Connected").await;
    assert!(
        saw_state(
            &events,
            peer,
            ConnState::Disconnected,
            ConnState::Connecting
        ),
        "first dial must record Connecting: {events:?}"
    );
    assert!(
        saw_state(&events, peer, ConnState::Connecting, ConnState::Connected),
        "dial success must land Connected: {events:?}"
    );
    assert_eq!(a.peer_state(&peer), Some(ConnState::Connected));

    // accept 循环持有 Arc<Swarm> 强引用：必须显式 shutdown 才真正关停并拆链
    b.shutdown();
    wait_for(&mut ev, "Connected -> BackingOff", |ev| {
        matches!(ev, LifecycleEvent::PeerStateChanged { peer: p, from: ConnState::Connected, to: ConnState::BackingOff } if *p == peer)
    })
    .await;
    wait_for(&mut ev, "BackingOff -> Connecting", |ev| {
        matches!(ev, LifecycleEvent::PeerStateChanged { peer: p, from: ConnState::BackingOff, to: ConnState::Connecting } if *p == peer)
    })
    .await;
    wait_for(&mut ev, "reconnect fail -> BackingOff", |ev| {
        matches!(ev, LifecycleEvent::PeerStateChanged { peer: p, from: ConnState::Connecting, to: ConnState::BackingOff } if *p == peer)
    })
    .await;
    a.shutdown();
}

/// 验收 2：失聪对端连续未命中判定离线，PeerDown 事件 + Connected→BackingOff；
/// 自动重连成功后 PeerUp（对端活着，可恢复）。
#[tokio::test]
async fn probe_misses_judge_peer_down_then_recovers() {
    let a = Swarm::start_with_lifecycle(
        config(Keypair::generate(), HandlerRegistry::default()),
        lifecycle(),
    )
    .await
    .expect("bind a");
    let mut deaf_registry = HandlerRegistry::default();
    deaf_registry.register(Arc::new(Deaf));
    let b = Swarm::start_with_lifecycle(config(Keypair::generate(), deaf_registry), lifecycle())
        .await
        .expect("bind b");
    let peer = b.local_peer_id();
    a.add_peer_addresses(peer, b.listen_addrs());
    let mut ev = a.subscribe_lifecycle();
    a.connect(peer).await.expect("dial b");

    let events = wait_for(
        &mut ev,
        "PeerDown",
        |ev| matches!(ev, LifecycleEvent::PeerDown { peer: p, .. } if *p == peer),
    )
    .await;
    assert!(
        saw_state(&events, peer, ConnState::Connected, ConnState::BackingOff),
        "probe down must transition to BackingOff: {events:?}"
    );
    wait_for(
        &mut ev,
        "PeerUp after auto-reconnect",
        |ev| matches!(ev, LifecycleEvent::PeerUp { peer: p } if *p == peer),
    )
    .await;
    a.shutdown();
    b.shutdown();
}

/// 验收 3（复位语义，消融锚点）：健康会话（>= reset_min_uptime）后的重连成功
/// 把退避归零——下次断链排定退避回到 base。注释掉 mark_connected 的复位块后，
/// 本用例在最后一行断言变红（退避序列继续爬升为 4×base）。
#[tokio::test]
async fn backoff_resets_after_healthy_reconnect() {
    let seed = Keypair::generate();
    let a = Swarm::start_with_lifecycle(
        config(Keypair::generate(), HandlerRegistry::default()),
        lifecycle_without_probe(),
    )
    .await
    .expect("bind a");
    let b = Swarm::start_with_lifecycle(
        config(
            Keypair::from_seed(&seed.to_seed_bytes()),
            HandlerRegistry::default(),
        ),
        lifecycle_without_probe(),
    )
    .await
    .expect("bind b");
    let peer = b.local_peer_id();
    a.add_peer_addresses(peer, tcp_addrs(&b));
    let mut ev = a.subscribe_lifecycle();
    a.connect(peer).await.expect("dial b");
    wait_connected(&mut ev, peer, "initial Connected").await;
    // S1 健康期：存活必须 >= reset_min_uptime，才配得上复位
    tokio::time::sleep(RESET_MIN_UPTIME + Duration::from_millis(100)).await;

    // 同前：显式 shutdown 才会关监听、拆连接（accept 循环持强引用）
    b.shutdown();
    wait_for(&mut ev, "S1 drop -> BackingOff", |ev| {
        matches!(ev, LifecycleEvent::PeerStateChanged { peer: p, from: ConnState::Connected, to: ConnState::BackingOff } if *p == peer)
    })
    .await;
    assert_eq!(
        a.peer_scheduled_backoff(&peer),
        Some(BASE),
        "first outage must schedule base backoff"
    );
    wait_for(&mut ev, "retry1 due", |ev| {
        matches!(ev, LifecycleEvent::PeerStateChanged { peer: p, from: ConnState::BackingOff, to: ConnState::Connecting } if *p == peer)
    })
    .await;
    wait_for(&mut ev, "retry1 failed", |ev| {
        matches!(ev, LifecycleEvent::PeerStateChanged { peer: p, from: ConnState::Connecting, to: ConnState::BackingOff } if *p == peer)
    })
    .await;
    assert_eq!(
        a.peer_scheduled_backoff(&peer),
        Some(BASE * 2),
        "failed retry must double the backoff"
    );

    // 对端同身份复活：地址簿补新地址，下一次重连经旧地址失败后在新地址成功
    let b2 = Swarm::start_with_lifecycle(
        config(
            Keypair::from_seed(&seed.to_seed_bytes()),
            HandlerRegistry::default(),
        ),
        lifecycle_without_probe(),
    )
    .await
    .expect("bind b2");
    a.add_peer_addresses(peer, tcp_addrs(&b2));
    wait_connected(&mut ev, peer, "reconnect Connected").await;
    // 复位触发点在新连接建成时已执行（S1 健康）。S2 闪断后退避必须回到 base。
    b2.shutdown();
    wait_for(&mut ev, "S2 drop -> BackingOff", |ev| {
        matches!(ev, LifecycleEvent::PeerStateChanged { peer: p, from: ConnState::Connected, to: ConnState::BackingOff } if *p == peer)
    })
    .await;
    assert_eq!(
        a.peer_scheduled_backoff(&peer),
        Some(BASE),
        "healthy-session reset must restore base backoff (ablation: this is 4x base)"
    );
    a.shutdown();
}
