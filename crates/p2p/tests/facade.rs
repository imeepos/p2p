//! facade 验收（coordination.md S 包）：两 Node 经 facade 互拨 request roundtrip，
//! 事件可见（PeerDiscovered/PeerConnected/PeerDisconnected）；mdns 变体 #[ignore]。
//!
//! 断开/门禁用例只登记 TCP 地址：yamux 句柄归零即断链，关闭语义确定；
//! QUIC 监听端关停无法本地断开存量连接（quinn 驱动随存活连接续跑，见最终报告）。
//! 每条连接只做一次开流：yamux 驱动在空闲连接上的第二次 open_stream 存在
//! 唤醒丢失缺陷（p2p-mux 上游问题，已报告），多流场景待上游修复后补验收。

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use p2p::{Node, NodeEvent, PeerId, ProtocolHandler, ProtocolId};
use p2p_mux::BoxedStream;
use p2p_protocol::{read_frame, write_frame};
use tokio::sync::broadcast;

const ECHO_PROTOCOL: &str = "/test/echo/1";
const WAIT: Duration = Duration::from_secs(10);

struct Echo;

#[async_trait::async_trait]
impl ProtocolHandler for Echo {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(ECHO_PROTOCOL).expect("valid protocol id")
    }

    async fn handle(&self, mut stream: BoxedStream) -> io::Result<()> {
        let req = read_frame(&mut stream).await?;
        write_frame(&mut stream, &req).await
    }
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-facade-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    dir
}

async fn expect_event(
    rx: &mut broadcast::Receiver<NodeEvent>,
    want: &dyn Fn(&NodeEvent) -> bool,
    what: &'static str,
) -> NodeEvent {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let budget = deadline.saturating_duration_since(tokio::time::Instant::now());
        let ev = tokio::time::timeout(budget, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
            .expect("event channel open");
        if want(&ev) {
            return ev;
        }
    }
}

/// 只登记 target 的指定传输地址（"/u"=QUIC，"/t"=TCP；None=全部）。
fn seed_addrs(target: &Node, peer: PeerId, marker: Option<&str>, into: &Node) {
    for addr in target.listen_addrs() {
        if let Some(m) = marker {
            if !addr.contains(m) {
                continue;
            }
        }
        into.add_peer_address(peer, &addr).expect("seed addr");
    }
}

/// 主验收：显式地址直连，request roundtrip + 发现/连接/断开三类事件可见。
#[tokio::test]
async fn two_nodes_roundtrip_with_events() {
    let a_dir = tmp_dir("a");
    let b_dir = tmp_dir("b");
    let node_a = Node::builder()
        .mdns(false)
        .data_dir(a_dir.clone())
        .build()
        .await
        .expect("node a");
    let node_b = Node::builder()
        .mdns(false)
        .data_dir(b_dir.clone())
        .build()
        .await
        .expect("node b");
    let a_peer = node_a.local_peer_id();

    node_a.handle_protocol(Arc::new(Echo));

    // 先订阅再登记地址，保证事件不丢
    let mut events_b = node_b.events();
    seed_addrs(&node_a, a_peer, Some("/t"), &node_b);

    let discovered = expect_event(
        &mut events_b,
        &|ev| matches!(ev, NodeEvent::PeerDiscovered { peer, .. } if *peer == a_peer),
        "PeerDiscovered",
    )
    .await;
    assert!(
        matches!(&discovered, NodeEvent::PeerDiscovered { addrs, .. } if !addrs.is_empty()),
        "discovered addrs must be visible"
    );

    node_b.connect(a_peer).await.expect("connect a");
    expect_event(
        &mut events_b,
        &|ev| matches!(ev, NodeEvent::PeerConnected { peer } if *peer == a_peer),
        "PeerConnected",
    )
    .await;

    let payload = b"hello swarm".to_vec();
    let reply = node_b
        .request(
            a_peer,
            ProtocolId::new(ECHO_PROTOCOL).expect("id"),
            payload.clone(),
            Duration::from_secs(5),
        )
        .await
        .expect("echo roundtrip");
    assert_eq!(reply, payload);

    // 断开 A：B 侧必须看到 PeerDisconnected（断开路径可见，design §12）
    node_a.shutdown();
    expect_event(
        &mut events_b,
        &|ev| matches!(ev, NodeEvent::PeerDisconnected { peer } if *peer == a_peer),
        "PeerDisconnected",
    )
    .await;

    node_b.shutdown();
    let _ = std::fs::remove_dir_all(&a_dir);
    let _ = std::fs::remove_dir_all(&b_dir);
}

/// new_stream 路径：首帧协议 ID 已由 facade 写入，业务帧直收（独立节点对）。
#[tokio::test]
async fn new_stream_roundtrip() {
    let a_dir = tmp_dir("ns-a");
    let b_dir = tmp_dir("ns-b");
    let node_a = Node::builder()
        .mdns(false)
        .data_dir(a_dir.clone())
        .build()
        .await
        .expect("node a");
    let node_b = Node::builder()
        .mdns(false)
        .data_dir(b_dir.clone())
        .build()
        .await
        .expect("node b");
    let a_peer = node_a.local_peer_id();

    node_a.handle_protocol(Arc::new(Echo));
    seed_addrs(&node_a, a_peer, Some("/t"), &node_b);

    node_b.connect(a_peer).await.expect("connect a");
    let mut stream = node_b
        .new_stream(a_peer, ProtocolId::new(ECHO_PROTOCOL).expect("id"))
        .await
        .expect("new stream");
    write_frame(&mut stream, b"via-stream")
        .await
        .expect("write");
    let echoed = read_frame(&mut stream).await.expect("read");
    assert_eq!(echoed, b"via-stream");

    node_a.shutdown();
    node_b.shutdown();
    let _ = std::fs::remove_dir_all(&a_dir);
    let _ = std::fs::remove_dir_all(&b_dir);
}

/// QUIC 直连路径：只登记 QUIC 地址（无 TCP 兜底），roundtrip 成功即证 QUIC 拨号可用。
#[tokio::test]
async fn quic_direct_roundtrip() {
    let a_dir = tmp_dir("quic-a");
    let b_dir = tmp_dir("quic-b");
    let node_a = Node::builder()
        .mdns(false)
        .data_dir(a_dir.clone())
        .build()
        .await
        .expect("node a");
    let node_b = Node::builder()
        .mdns(false)
        .data_dir(b_dir.clone())
        .build()
        .await
        .expect("node b");
    let a_peer = node_a.local_peer_id();

    node_a.handle_protocol(Arc::new(Echo));
    seed_addrs(&node_a, a_peer, Some("/u"), &node_b);

    let mut events_b = node_b.events();
    node_b.connect(a_peer).await.expect("quic connect");
    expect_event(
        &mut events_b,
        &|ev| matches!(ev, NodeEvent::PeerConnected { peer } if *peer == a_peer),
        "PeerConnected",
    )
    .await;

    let reply = node_b
        .request(
            a_peer,
            ProtocolId::new(ECHO_PROTOCOL).expect("id"),
            b"quic".to_vec(),
            Duration::from_secs(5),
        )
        .await
        .expect("quic roundtrip");
    assert_eq!(reply, b"quic");

    // QUIC 关停无法本地断开存量连接（上游 QuicTransport 无 close 接口），此处不断言断开事件
    node_a.shutdown();
    node_b.shutdown();
    let _ = std::fs::remove_dir_all(&a_dir);
    let _ = std::fs::remove_dir_all(&b_dir);
}

/// 门禁拒收：A 拒绝 B 后断链，B 侧看到 PeerDisconnected 且后续请求失败。
#[tokio::test]
async fn gate_denied_inbound_is_dropped() {
    let a_dir = tmp_dir("gate-a");
    let b_dir = tmp_dir("gate-b");
    let node_a = Node::builder()
        .mdns(false)
        .data_dir(a_dir.clone())
        .build()
        .await
        .expect("node a");
    let node_b = Node::builder()
        .mdns(false)
        .data_dir(b_dir.clone())
        .build()
        .await
        .expect("node b");
    let a_peer = node_a.local_peer_id();

    node_a.set_gate(Arc::new(p2p_swarm::gate_fn(|_| false)));
    node_a.handle_protocol(Arc::new(Echo));

    let mut events_b = node_b.events();
    seed_addrs(&node_a, a_peer, Some("/t"), &node_b);

    node_b.connect(a_peer).await.expect("dial completes");
    expect_event(
        &mut events_b,
        &|ev| matches!(ev, NodeEvent::PeerDisconnected { peer } if *peer == a_peer),
        "gated PeerDisconnected",
    )
    .await;

    let outcome = node_b
        .request(
            a_peer,
            ProtocolId::new(ECHO_PROTOCOL).expect("id"),
            b"denied".to_vec(),
            Duration::from_secs(5),
        )
        .await;
    assert!(outcome.is_err(), "request over gated-closed link must fail");

    node_a.shutdown();
    node_b.shutdown();
    let _ = std::fs::remove_dir_all(&a_dir);
    let _ = std::fs::remove_dir_all(&b_dir);
}
