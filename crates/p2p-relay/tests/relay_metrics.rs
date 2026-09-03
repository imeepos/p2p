//! E8-M2 指标埋点回归：建立/拒绝/回收计数随事件递增、保活失败计数、桥接字节。
//! 消融点：删除 control.rs 静默分支 count_keepalive_failure、circuit.rs
//! count_connect_reject / count_idle_reclaimed / add_bridged_bytes、
//! lifecycle.rs count_recycled 任一计数调用，对应用例即红。

use std::sync::Arc;
use std::time::Duration;

use p2p_mux::BoxedStream;
use p2p_relay::testutil::{mock_link_pair, MockLinkSource};
use p2p_relay::{
    errcode, read_msg, relay_msg::Kind, write_msg, CircuitId, RelayClient, RelayKeepalive,
    RelayLimits, RelayLink, RelayMetricsSnapshot, RelayMsg, RelayService, RelayServiceImpl,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn secs(s: u64) -> Duration {
    Duration::from_secs(s)
}

/// 全字段显式的保活参数（与 relay_stability 同口径），各用例只调关心的旋钮。
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

/// 进程内 relay + 双客户端 + 服务句柄；返回 source 供调用方保活。
fn relay_pair(
    ka: RelayKeepalive,
    limits: RelayLimits,
) -> (
    Arc<RelayServiceImpl>,
    RelayClient,
    RelayClient,
    MockLinkSource,
) {
    let source = MockLinkSource::new();
    let (ca, sa) = mock_link_pair("peer-a", "peer-a");
    let (cb, sb) = mock_link_pair("peer-b", "peer-b");
    source.push(Box::new(sa));
    source.push(Box::new(sb));
    let svc = spawn_relay(&source, limits, ka.clone());
    (
        svc,
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

/// 有界读一帧：断言失败时不挂死，超时即 panic 变红。
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

/// 有界轮询指标直到条件成立；超时携带末次快照 panic（事件必须体现为计数递增）。
async fn wait_until(
    within: Duration,
    what: &str,
    mut probe: impl FnMut() -> RelayMetricsSnapshot,
    cond: impl Fn(&RelayMetricsSnapshot) -> bool,
) -> RelayMetricsSnapshot {
    let deadline = tokio::time::Instant::now() + within;
    loop {
        let snap = probe();
        if cond(&snap) {
            return snap;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{what} not reached; last snapshot {snap:?}"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// 建立/拒绝计数与在路 gauge 随事件递增：发放 +1、配额拒 +1、connect 拒 +1。
#[tokio::test]
async fn issue_reject_counters_and_gauge_track_events() {
    let source = MockLinkSource::new();
    let limits = RelayLimits {
        max_circuits_per_peer: 1,
        ..RelayLimits::default()
    };
    let svc = spawn_relay(&source, limits, RelayKeepalive::default());
    let (cl, sl) = mock_link_pair("peer-a", "peer-a");
    source.push(Box::new(sl));
    let mut ctrl = cl.open_stream().await.expect("open control");

    let cid = manual_reserve(&mut ctrl, 3600, "").await;
    let snap = wait_until(
        secs(2),
        "issued_total=1",
        || svc.metrics(),
        |s| s.circuits_issued_total == 1,
    )
    .await;
    assert_eq!(snap.circuits_active, 1, "在路 gauge 须随发放上升");
    assert_eq!(snap.controls_registered, 1);

    // per-peer 配额已满：再次 reserve 得显式 PEER_LIMIT，拒绝计数递增
    write_msg(&mut ctrl, &RelayMsg::reserve(3600, ""))
        .await
        .expect("second reserve write");
    match read_frame(&mut ctrl).await.kind {
        Some(Kind::Reject(r)) => assert_eq!(r.code, errcode::PEER_LIMIT, "got {r:?}"),
        other => panic!("expected peer-limit reject: {other:?}"),
    }
    wait_until(
        secs(2),
        "reserve_rejects_total=1",
        || svc.metrics(),
        |s| s.reserve_rejects_total == 1,
    )
    .await;

    // 未知电路 connect 得显式 UNKNOWN_CIRCUIT，connect 拒绝计数递增
    let mut s = cl.open_stream().await.expect("open circuit stream");
    write_msg(&mut s, &RelayMsg::connect(987_654_321))
        .await
        .expect("write connect");
    match read_frame(&mut s).await.kind {
        Some(Kind::Reject(r)) => assert_eq!(r.code, errcode::UNKNOWN_CIRCUIT, "got {r:?}"),
        other => panic!("expected unknown-circuit reject: {other:?}"),
    }
    let _ = cid;
    wait_until(
        secs(2),
        "connect_rejects_total=1",
        || svc.metrics(),
        |s| s.connect_rejects_total == 1,
    )
    .await;
}

/// 空闲回收计数：拆桥后 bridges_idle_reclaimed_total 递增（E6-R3 消融点 1 路径）。
#[tokio::test]
async fn idle_reclaim_counter_increments_when_bridge_reclaimed() {
    let limits = RelayLimits {
        max_circuits_per_peer: 2,
        ..RelayLimits::default()
    };
    let (svc, mut a, mut b, _keep) = relay_pair(ka(250, 10_000, 5_000, 3, 45_000), limits);
    let (mut sa, mut sb) = bridged_pair(&mut a, &mut b).await;
    let drain = async {
        let (mut ba, mut bb) = ([0u8; 4], [0u8; 4]);
        tokio::join!(sa.read(&mut ba), sb.read(&mut bb))
    };
    let converged = tokio::time::timeout(secs(5), drain)
        .await
        .expect("idle reclaim must close bridge in 5s");
    for r in [converged.0, converged.1] {
        assert!(r.is_err() || r.unwrap_or(0) == 0, "idle bridge must close");
    }
    let snap = wait_until(
        secs(2),
        "idle_reclaimed=1",
        || svc.metrics(),
        |s| s.bridges_idle_reclaimed_total == 1,
    )
    .await;
    assert_eq!(snap.circuits_active, 0, "拆桥后槽位随退役归零");
}

/// 信令面消失回收计数：控制流关闭后 circuits_recycled_total 递增（E6-R3 流级触发器）。
#[tokio::test]
async fn recycled_counter_increments_when_signaling_gone() {
    let source = MockLinkSource::new();
    let svc = spawn_relay(&source, RelayLimits::default(), RelayKeepalive::default());
    let (cl, sl) = mock_link_pair("churn-a", "churn-a");
    source.push(Box::new(sl));
    let mut ctrl = cl.open_stream().await.expect("open control");
    manual_reserve(&mut ctrl, 3600, "").await;
    assert_eq!(svc.metrics().circuits_active, 1);
    drop(ctrl);
    let snap = wait_until(
        secs(2),
        "recycled_total=1",
        || svc.metrics(),
        |s| s.circuits_recycled_total == 1,
    )
    .await;
    assert_eq!(snap.circuits_active, 0, "回收后水位必须归零");
}

/// 保活失败计数：客户端静默超 server_silence 被清理后 keepalive_failures_total 递增。
#[tokio::test]
async fn keepalive_failure_counter_increments_on_silence_timeout() {
    let source = MockLinkSource::new();
    let svc = spawn_relay(
        &source,
        RelayLimits::default(),
        ka(120_000, 10_000, 5_000, 3, 300),
    );
    let (cl, sl) = mock_link_pair("silent-a", "silent-a");
    source.push(Box::new(sl));
    let mut ctrl = cl.open_stream().await.expect("open control");
    manual_reserve(&mut ctrl, 3600, "").await;
    let snap = wait_until(
        secs(3),
        "keepalive_failures_total=1",
        || svc.metrics(),
        |s| s.keepalive_failures_total == 1,
    )
    .await;
    assert_eq!(snap.circuits_active, 0, "保活失败的客户端电路须随清理回收");
}

/// 桥接字节计数：正常关桥后 bridged_bytes_total 等于双向搬运量，且不误记空闲回收。
#[tokio::test]
async fn bridged_bytes_counter_tracks_payload() {
    let (svc, mut a, mut b, _keep) = relay_pair(
        ka(120_000, 10_000, 5_000, 3, 45_000),
        RelayLimits::default(),
    );
    let (mut sa, mut sb) = bridged_pair(&mut a, &mut b).await;
    pump(&mut sa, &mut sb, &[7u8; 48]).await;
    pump(&mut sb, &mut sa, &[9u8; 16]).await;
    drop(sa);
    drop(sb);
    let snap = wait_until(
        secs(3),
        "bridged_bytes_total>=64",
        || svc.metrics(),
        |s| s.bridged_bytes_total >= 64,
    )
    .await;
    assert_eq!(
        snap.bridges_idle_reclaimed_total, 0,
        "正常关桥不得记为空闲回收"
    );
}
