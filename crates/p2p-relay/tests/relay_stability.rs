//! E6 稳定性回归：空闲回收 / 保活失联 / 服务端静默清理 / 既有回收语义。
//! 消融点 1: circuit.rs supervise_bridge 空闲分支 -> idle_bridged_circuit_reclaimed_after_ttl
//! 消融点 2: client.rs spawn_keepalive -> keepalive_misses_declare_relay_lost_and_fail_fast
//! 消融点 3: control.rs control_loop timeout 包装 -> server_silence_timeout_reclaims_client_state

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use p2p_mux::BoxedStream;
use p2p_relay::testutil::{mock_link_pair, MockLinkSource};
use p2p_relay::{
    errcode, read_msg, relay_msg::Kind, write_msg, CircuitId, RelayClient, RelayError, RelayEvent,
    RelayKeepalive, RelayLimits, RelayLink, RelayMsg, RelayService, RelayServiceImpl,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn secs(s: u64) -> Duration {
    Duration::from_secs(s)
}

/// 全字段显式的保活参数，各用例只调关心的旋钮。
fn ka(idle: u64, itv: u64, to: u64, miss: u32, silence: u64) -> RelayKeepalive {
    RelayKeepalive {
        idle_circuit_ttl: Duration::from_millis(idle),
        interval: Duration::from_millis(itv),
        timeout: Duration::from_millis(to),
        max_missed: miss,
        server_silence: Duration::from_millis(silence),
    }
}

fn spawn_relay(
    source: &MockLinkSource,
    limits: RelayLimits,
    ka: RelayKeepalive,
) -> Arc<RelayServiceImpl> {
    let svc = Arc::new(RelayServiceImpl::with_keepalive(
        Box::new(source.clone()),
        limits,
        ka,
    ));
    let worker = svc.clone();
    tokio::spawn(async move {
        let _ = worker.serve().await;
    });
    svc
}

/// 进程内 relay + 双客户端；服务端侧链路 peer_id 必须是对端身份（配额/属主校验依据）。
fn relay_pair_with(
    ka: RelayKeepalive,
    limits: RelayLimits,
) -> (RelayClient, RelayClient, MockLinkSource) {
    let source = MockLinkSource::new();
    let (ca, sa) = mock_link_pair("peer-a", "peer-a");
    let (cb, sb) = mock_link_pair("peer-b", "peer-b");
    source.push(Box::new(sa));
    source.push(Box::new(sb));
    spawn_relay(&source, limits, ka.clone());
    (
        RelayClient::with_keepalive(Box::new(ca), ka.clone()),
        RelayClient::with_keepalive(Box::new(cb), ka),
        source,
    )
}

/// 双客户端 reserve + 双向接入，返回已桥接的两侧数据流。
async fn bridged_pair(a: &mut RelayClient, b: &mut RelayClient) -> (BoxedStream, BoxedStream) {
    let cid = a.reserve(secs(60), "peer-b").await.expect("reserve");
    let (sa, sb) = tokio::join!(a.connect(cid), b.connect(cid));
    (sa.expect("a connect"), sb.expect("b connect"))
}

/// 有界读一帧：消融变红时防用例挂死，超时即 panic。
async fn read_frame(r: &mut BoxedStream) -> RelayMsg {
    tokio::time::timeout(secs(3), read_msg(r))
        .await
        .expect("bounded frame read")
        .expect("read frame io")
        .expect("frame present")
}

/// 在裸控制流上完成 reserve 往返，返回发放的电路号。
async fn manual_reserve(ctrl: &mut BoxedStream, ttl: u64, joiner: &str) -> CircuitId {
    write_msg(ctrl, &RelayMsg::reserve(ttl, joiner))
        .await
        .expect("reserve write");
    match read_frame(ctrl).await.kind {
        Some(Kind::Reserved(r)) => CircuitId(r.circuit_id),
        other => panic!("expected Reserved: {other:?}"),
    }
}

/// 单向泵一段字节并断言逐字节一致。
async fn pump(a: &mut BoxedStream, b: &mut BoxedStream, payload: &[u8]) {
    let mut got = vec![0u8; payload.len()];
    let (w, r) = tokio::join!(a.write_all(payload), b.read_exact(&mut got));
    w.expect("pump write");
    r.expect("pump read");
    assert_eq!(got, payload);
}

/// 静默黑洞链路：开流返回永不读写的流（对端钉住不 EOF 不回包），accept 永挂。
struct BlackHoleLink {
    peer: String,
    parked: Mutex<Vec<BoxedStream>>,
}

impl BlackHoleLink {
    fn new(peer: &str) -> Self {
        Self {
            peer: peer.into(),
            parked: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl RelayLink for BlackHoleLink {
    fn peer_id(&self) -> &str {
        &self.peer
    }

    async fn open_stream(&self) -> io::Result<BoxedStream> {
        let (ours, theirs) = tokio::io::duplex(4096);
        self.parked.lock().expect("parked").push(Box::new(theirs));
        Ok(Box::new(ours))
    }

    async fn accept_stream(&self) -> Option<BoxedStream> {
        std::future::pending().await
    }
}

/// 空闲回收（含消融点 1）：双向静默超过 idle TTL 即拆桥，配额随槽位退役回吐。
#[tokio::test]
async fn idle_bridged_circuit_reclaimed_after_ttl() {
    // per-peer 配额 2：桥接期被双方打满（reserve+connect），回收后必须能再次发放
    let limits = RelayLimits {
        max_circuits_per_peer: 2,
        ..RelayLimits::default()
    };
    let (mut a, mut b, _keep) = relay_pair_with(ka(250, 10_000, 5_000, 3, 45_000), limits);
    let (mut sa, mut sb) = bridged_pair(&mut a, &mut b).await;
    // 持续静默：两侧流必须在 idle TTL 后收敛。消融点 1 注释掉回收分支后，
    // 此 read 永不返回 -> 5s 超时 panic 变红。
    let drain = async {
        let (mut ba, mut bb) = ([0u8; 4], [0u8; 4]);
        tokio::join!(sa.read(&mut ba), sb.read(&mut bb))
    };
    let converged = tokio::time::timeout(secs(5), drain)
        .await
        .expect("must reclaim in 5s");
    for r in [converged.0, converged.1] {
        assert!(r.is_err() || r.unwrap_or(0) == 0, "idle bridge must close");
    }
    // 配额回吐断言：消融后配额仍被占用，此处变红。
    a.reserve(secs(60), "peer-b").await.expect("quota back");
    b.reserve(secs(60), "").await.expect("quota back");
}

/// 反例：有数据流动的电路绝不被误收（静默间隙恒小于 idle TTL）。
#[tokio::test]
async fn active_bridged_circuit_never_reclaimed() {
    let (mut a, mut b, _keep) =
        relay_pair_with(ka(250, 10_000, 5_000, 3, 45_000), RelayLimits::default());
    let (mut sa, mut sb) = bridged_pair(&mut a, &mut b).await;
    // 每 ~100ms 双向各一帧，跨越 3 倍 idle TTL：任一刻静默 < TTL，桥必须存活。
    for i in 0..8u8 {
        pump(&mut sa, &mut sb, &[i; 16]).await;
        pump(&mut sb, &mut sa, &[i; 16]).await;
        tokio::time::sleep(Duration::from_millis(80)).await;
    }
    pump(&mut sa, &mut sb, b"still-alive").await;
}

/// 保活失联（含消融点 2）：连续超时判失联，WARN+事件上抛，控制面速断。
#[tokio::test]
async fn keepalive_misses_declare_relay_lost_and_fail_fast() {
    let ka = ka(120_000, 60, 60, 2, 45_000);
    let mut client = RelayClient::with_keepalive(Box::new(BlackHoleLink::new("black-hole")), ka);
    // 惰性控制流需先触发：reserve 在黑洞上 5s 才超时，不等它，只建流。
    let _ = tokio::time::timeout(Duration::from_millis(300), client.reserve(secs(60), "")).await;
    // 连续 2 次探测无应答 -> 失联事件。消融点 2 注释掉 spawn_keepalive 后，
    // 永无事件 -> 3s 超时变红。
    let ev = tokio::time::timeout(secs(3), client.next_event())
        .await
        .expect("loss must be declared within 3s")
        .expect("event queued");
    match ev {
        RelayEvent::ControlClosed { reason } => {
            assert!(reason.contains("keepalive"), "must be keepalive: {reason}");
        }
        other => panic!("unexpected event: {other:?}"),
    }
    // 失联后控制面速断：不许静默重开控制流装作没事。
    let retry = tokio::time::timeout(secs(2), client.reserve(secs(60), ""));
    let err = retry
        .await
        .expect("fail-fast must not hang")
        .expect_err("must fail");
    assert!(matches!(err, RelayError::LinkClosed), "got {err:?}");
}

/// 服务端静默清理（含消融点 3）+ 健康保活不误伤反例。
#[tokio::test]
async fn server_silence_timeout_reclaims_and_keepalive_survives() {
    let source = MockLinkSource::new();
    let limits = RelayLimits {
        max_circuits_per_peer: 1,
        ..RelayLimits::default()
    };
    let _svc = spawn_relay(&source, limits, ka(120_000, 10_000, 5_000, 3, 300));
    let (cl, sl) = mock_link_pair("silent-a", "silent-a");
    source.push(Box::new(sl));
    let mut ctrl = cl.open_stream().await.expect("open control");
    let cid = manual_reserve(&mut ctrl, 3600, "").await;
    // 裸控制流不发保活：超过 server_silence 服务端按失联清理。消融点 3 注释
    // 掉 timeout 包装后电路滞留，①读挂死超时、②配额自锁，双双变红。
    tokio::time::sleep(Duration::from_millis(600)).await;
    // ① 旧电路已被回收：connect 得显式 UNKNOWN_CIRCUIT（非挂死、非停车）。
    let mut s = cl.open_stream().await.expect("open circuit stream");
    write_msg(&mut s, &RelayMsg::connect(cid.0))
        .await
        .expect("write connect");
    match read_frame(&mut s).await.kind {
        Some(Kind::Reject(r)) => assert_eq!(r.code, errcode::UNKNOWN_CIRCUIT, "got {r:?}"),
        other => panic!("expected unknown-circuit reject: {other:?}"),
    }
    // ② 配额回吐：同 Peer 换新控制流可再次注册（否则 PEER_LIMIT）。
    let mut ctrl2 = cl.open_stream().await.expect("reopen control");
    let _ = manual_reserve(&mut ctrl2, 3600, "").await;
    // ③ 反例：健康客户端（50ms 保活）静默 1s（>2 倍 silence）控制流不得被清。
    let (hl, hs) = mock_link_pair("healthy-b", "healthy-b");
    source.push(Box::new(hs));
    let mut h = RelayClient::with_keepalive(Box::new(hl), ka(120_000, 50, 5_000, 3, 300));
    h.reserve(secs(3600), "").await.expect("healthy register");
    tokio::time::sleep(secs(1)).await;
    // 若被清服务端 EOF 必达客户端成 ControlClosed 事件；无事件即保活生效
    let ev = tokio::time::timeout(Duration::from_millis(500), h.next_event()).await;
    assert!(ev.is_err(), "healthy control must survive silence: {ev:?}");
}

/// 既有回收语义回归（E4）：控制流关闭回收未桥接电路，桥接中的不受牵连。
#[tokio::test]
async fn control_close_keeps_bridged_rejects_parked() {
    let source = MockLinkSource::new();
    let svc = spawn_relay(&source, RelayLimits::default(), RelayKeepalive::default());
    let (a_cli, a_srv) = mock_link_pair("peer-a", "peer-a");
    let (b_cli, b_srv) = mock_link_pair("peer-b", "peer-b");
    source.push(Box::new(a_srv));
    source.push(Box::new(b_srv));
    let mut b = RelayClient::new(Box::new(b_cli));
    // 裸控制流注册 + 裸流停车 + b 客户端接入配对成桥
    let mut ctrl = a_cli.open_stream().await.expect("open control");
    let cid = manual_reserve(&mut ctrl, 3600, "peer-b").await;
    let mut sa = a_cli.open_stream().await.expect("open circuit stream");
    write_msg(&mut sa, &RelayMsg::connect(cid.0))
        .await
        .expect("write connect");
    let mut sb = b.connect(cid).await.expect("b joins circuit");
    assert!(
        matches!(read_frame(&mut sa).await.kind, Some(Kind::Bound(_))),
        "expected Bound"
    );
    pump(&mut sa, &mut sb, b"before-close").await;
    // 控制流关闭（EOF）：桥接电路不受牵连（E4），数据继续互通
    drop(ctrl);
    pump(&mut sa, &mut sb, b"after-close").await;
    // 未桥接电路随控制流关闭回收（E4）。停车处理与 EOF 竞态落在哪一侧都被
    // 回收：停车先处理则持车方收显式 CIRCUIT_EXPIRED，EOF 先处理则在途
    // connect 收 UNKNOWN_CIRCUIT——两者都是显式错误信号而非静默悬挂。
    let mut ctrl2 = a_cli.open_stream().await.expect("reopen control");
    let cid2 = manual_reserve(&mut ctrl2, 3600, "").await;
    assert_eq!(svc.metrics().circuits_active, 2, "cid2 slot registered");
    let mut parked = a_cli.open_stream().await.expect("open parked stream");
    write_msg(&mut parked, &RelayMsg::connect(cid2.0))
        .await
        .expect("write connect 2");
    drop(ctrl2);
    // 轮询槽位水位 2 -> 1：确定性等待回收完成（不依赖睡多久）
    for _ in 0..400 {
        if svc.metrics().circuits_active == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(svc.metrics().circuits_active, 1, "watermark never reached");
    match read_frame(&mut parked).await.kind {
        Some(Kind::Reject(r)) => assert!(
            r.code == errcode::CIRCUIT_EXPIRED || r.code == errcode::UNKNOWN_CIRCUIT,
            "expected explicit reclaim signal, got {r:?}"
        ),
        other => panic!("expected explicit reject, got {other:?}"),
    }
}
