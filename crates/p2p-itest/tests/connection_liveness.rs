//! 连接活性回归（2026-09 GUI「拨通即闪断」）：
//! 1. 拨号成功后连接必须保持，禁止无因 PeerDisconnected；
//! 2. 双向并发拨号必须收敛为一条连接，双向 request 都可用；
//! 3. 发现条目过期不得谎报活连接断开，但对真离线对端仍须发信号。
//!
//! 三条在修复前分别红：过期谎报 / 双拨分家单向流死 / 离线信号缺失无覆盖。

use std::io;
use std::sync::Arc;
use std::time::Duration;

use p2p_identity::{Keypair, PeerId};
use p2p_mux::BoxedStream;
use p2p_protocol::{read_frame, write_frame, HandlerRegistry, ProtocolHandler, ProtocolId};
use p2p_swarm::{NodeEvent, Swarm, SwarmConfig};
use tokio::sync::broadcast;

const ECHO: &str = "/itest/echo/1";
const SETTLE: Duration = Duration::from_secs(1);
const CONVERGE: Duration = Duration::from_millis(500);
const WINDOW: Duration = Duration::from_secs(3);
const ECHO_TIMEOUT: Duration = Duration::from_secs(3);

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

/// a、b 两个 swarm；两侧都注册 echo（双向 echo 各依赖对端 handler）；
/// a 登记全部地址（QUIC 在前，同直连排序）。
async fn pair() -> (Arc<Swarm>, Arc<Swarm>, PeerId) {
    let a = Swarm::start(config()).await.expect("bind a");
    let b = Swarm::start(config()).await.expect("bind b");
    a.register(Arc::new(Echo));
    b.register(Arc::new(Echo));
    let peer_b = b.local_peer_id();
    a.add_peer_addresses(peer_b, b.listen_addrs());
    (a, b, peer_b)
}

/// 建连后做一次 echo 往返，任何一步失败即 panic（带方向标签）。
async fn echo_roundtrip(swarm: &Swarm, peer: PeerId, what: &str) {
    let id = ProtocolId::new(ECHO).expect("valid");
    let raw = swarm
        .open_stream(&peer, &id)
        .await
        .unwrap_or_else(|e| panic!("{what}: open_stream: {e}"));
    let mut stream = p2p_protocol::open_with_protocol(raw, &id)
        .await
        .unwrap_or_else(|e| panic!("{what}: protocol handshake: {e}"));
    write_frame(&mut stream, b"ping")
        .await
        .unwrap_or_else(|e| panic!("{what}: write: {e}"));
    let reply = tokio::time::timeout(ECHO_TIMEOUT, read_frame(&mut stream))
        .await
        .unwrap_or_else(|_| panic!("{what}: echo reply timed out"))
        .unwrap_or_else(|e| panic!("{what}: read reply: {e}"));
    assert_eq!(reply, b"ping", "{what}: echo payload mismatch");
}

/// 在窗口期内收集事件（阻塞等待直至窗口关闭，禁止 0 窗口假收集）。
async fn drain(rx: &mut broadcast::Receiver<NodeEvent>, window: Duration) -> Vec<NodeEvent> {
    let deadline = tokio::time::Instant::now() + window;
    let mut out = Vec::new();
    loop {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Ok(ev)) => out.push(ev),
            _ => return out,
        }
    }
}

fn disconnected(events: &[NodeEvent], peer: PeerId) -> bool {
    events
        .iter()
        .any(|ev| matches!(ev, NodeEvent::PeerDisconnected { peer: p } if *p == peer))
}

/// 回归1：单次拨号成功后连接保持——窗口内零断开事件，回程 echo 可用。
#[tokio::test]
async fn dial_stays_connected_after_success() {
    let (a, b, peer) = pair().await;
    let mut ev_a = a.subscribe();
    let mut ev_b = b.subscribe();
    a.connect(peer).await.expect("dial b");
    let _ = drain(&mut ev_a, SETTLE).await;
    let _ = drain(&mut ev_b, SETTLE).await;

    let flash_a = drain(&mut ev_a, WINDOW).await;
    let flash_b = drain(&mut ev_b, WINDOW).await;
    assert!(
        !disconnected(&flash_a, peer),
        "dialer must not see disconnect after success: {flash_a:?}"
    );
    assert!(
        !disconnected(&flash_b, a.local_peer_id()),
        "acceptor must not see disconnect either: {flash_b:?}"
    );
    echo_roundtrip(&a, peer, "post-hold").await;
}

/// 回归2：双向并发拨号收敛为一条连接——零断开事件、双向 echo 可用、两侧池各剩 1。
#[tokio::test]
async fn mutual_dial_converges_to_one_working_connection() {
    let (a, b, peer_b) = pair().await;
    let peer_a = a.local_peer_id();
    b.add_peer_addresses(peer_a, a.listen_addrs());
    let mut ev_a = a.subscribe();
    let mut ev_b = b.subscribe();
    let (ra, rb) = tokio::join!(a.connect(peer_b), b.connect(peer_a));
    ra.expect("a dial b");
    rb.expect("b dial a");

    // 收敛窗口（毫秒级握手往返）：期间双方各持自己拨出的连接，跨端 accept
    // 落地后才收敛为同一条。窗口内新开流可能被落选侧复位（一次重试即好，
    // 修复前是永久单向断流）；断言从收敛后开始，闪断事件断言则覆盖全程。
    let _ = drain(&mut ev_a, CONVERGE).await;
    let _ = drain(&mut ev_b, CONVERGE).await;
    let flash_a = drain(&mut ev_a, WINDOW).await;
    let flash_b = drain(&mut ev_b, WINDOW).await;
    assert!(
        !disconnected(&flash_a, peer_b),
        "a must not flash disconnect on mutual dial: {flash_a:?}"
    );
    assert!(
        !disconnected(&flash_b, peer_a),
        "b must not flash disconnect on mutual dial: {flash_b:?}"
    );
    echo_roundtrip(&a, peer_b, "a->b").await;
    echo_roundtrip(&b, peer_a, "b->a").await;
    assert_eq!(
        a.metrics().active_connections,
        1,
        "a pool must converge to 1"
    );
    assert_eq!(
        b.metrics().active_connections,
        1,
        "b pool must converge to 1"
    );
}

/// 回归3：发现条目过期不得断开活连接——无断开事件且连接仍可用。
#[tokio::test]
async fn discovery_expiry_keeps_live_connection() {
    let (a, _b, peer) = pair().await;
    let mut ev_a = a.subscribe();
    a.connect(peer).await.expect("dial b");
    let _ = drain(&mut ev_a, SETTLE).await;

    a.on_peer_expired(peer);
    let events = drain(&mut ev_a, Duration::from_secs(1)).await;
    assert!(
        !disconnected(&events, peer),
        "discovery expiry must not lie about a live connection: {events:?}"
    );
    echo_roundtrip(&a, peer, "post-expiry").await;
}

/// 回归4：过期信号对真离线对端仍然有效——无连接时必须发 PeerDisconnected。
#[tokio::test]
async fn discovery_expiry_still_reports_offline_peer() {
    let (a, b, peer) = pair().await;
    let mut ev_a = a.subscribe();
    let _ = b; // 对端活着但本端从未连接：过期即视为离线
    a.on_peer_expired(peer);
    let events = drain(&mut ev_a, Duration::from_secs(1)).await;
    assert!(
        disconnected(&events, peer),
        "expiry without a live connection must report offline: {events:?}"
    );
}
