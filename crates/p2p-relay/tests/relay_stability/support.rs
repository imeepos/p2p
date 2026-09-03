//! 测试支撑件：保活参数、进程内 relay 装配、桥接/采样助手、黑洞链路。

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use p2p_mux::BoxedStream;
use p2p_relay::testutil::{mock_link_pair, MockLinkSource};
use p2p_relay::{
    read_msg, write_msg, CircuitId, RelayClient, RelayKeepalive, RelayLimits, RelayLink, RelayMsg,
    RelayService, RelayServiceImpl,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 全字段显式的保活参数，各用例只调关心的旋钮。
pub(crate) fn ka(idle: u64, itv: u64, to: u64, miss: u32, silence: u64) -> RelayKeepalive {
    RelayKeepalive {
        idle_circuit_ttl: Duration::from_millis(idle),
        interval: Duration::from_millis(itv),
        timeout: Duration::from_millis(to),
        max_missed: miss,
        server_silence: Duration::from_millis(silence),
    }
}

pub(crate) fn spawn_relay(
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
        let _ = RelayService::serve(worker).await;
    });
    svc
}

/// 进程内 relay + 双客户端；服务端侧链路 peer_id 必须是对端身份（配额/属主校验依据）。
pub(crate) fn relay_pair_with(
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
pub(crate) async fn bridged_pair(
    a: &mut RelayClient,
    b: &mut RelayClient,
) -> (BoxedStream, BoxedStream) {
    let cid = a
        .reserve(Duration::from_secs(60), "peer-b")
        .await
        .expect("reserve");
    let (sa, sb) = tokio::join!(a.connect(cid), b.connect(cid));
    (sa.expect("a connect"), sb.expect("b connect"))
}

/// 有界读一帧：消融变红时防用例挂死，超时即 panic。
pub(crate) async fn read_frame(r: &mut BoxedStream) -> RelayMsg {
    tokio::time::timeout(Duration::from_secs(3), read_msg(r))
        .await
        .expect("bounded frame read")
        .expect("read frame io")
        .expect("frame present")
}

/// 在裸控制流上完成 reserve 往返，返回发放的电路号。
pub(crate) async fn manual_reserve(ctrl: &mut BoxedStream, ttl: u64, joiner: &str) -> CircuitId {
    write_msg(ctrl, &RelayMsg::reserve(ttl, joiner))
        .await
        .expect("reserve write");
    match read_frame(ctrl).await.kind {
        Some(p2p_relay::relay_msg::Kind::Reserved(r)) => CircuitId(r.circuit_id),
        other => panic!("expected Reserved: {other:?}"),
    }
}

/// 单向泵一段字节并断言逐字节一致。
pub(crate) async fn pump(a: &mut BoxedStream, b: &mut BoxedStream, payload: &[u8]) {
    let mut got = vec![0u8; payload.len()];
    let (w, r) = tokio::join!(a.write_all(payload), b.read_exact(&mut got));
    w.expect("pump write");
    r.expect("pump read");
    assert_eq!(got, payload);
}

/// 静默黑洞链路：开流返回永不读写的流（对端钉住不 EOF 不回包），accept 永挂。
pub(crate) struct BlackHoleLink {
    peer: String,
    parked: Mutex<Vec<BoxedStream>>,
}

impl BlackHoleLink {
    pub(crate) fn new(peer: &str) -> Self {
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
