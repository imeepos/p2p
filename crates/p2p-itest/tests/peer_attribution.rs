//! 双对端并发归属（底座随流下传 peer 契约，ISSUE 2026-09-05 底座契约缺口）：
//! 两个对端并发各开一条流，handler 收到的身份各归各流互不串扰——
//! 每条流的 (归属 peer, 流上标记) 配对与真实拨号方一一对应。

use std::io;
use std::sync::Arc;
use std::time::Duration;

use p2p::{BoxedStream, Node, PeerId, ProtocolHandler, ProtocolId};
use p2p_protocol::{read_frame, write_frame};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

const PROTO: &str = "/itest/attribution/1";
/// 本地 loopback 全链毫秒级，10s 是宽松护栏。
const STEP: Duration = Duration::from_secs(10);

/// 身份捕获回显：记录分发层下传的 (归属 peer, 流上标记) 后原样回写。
struct AttributionEcho {
    proto: ProtocolId,
    seen: mpsc::UnboundedSender<(PeerId, String)>,
}

#[async_trait::async_trait]
impl ProtocolHandler for AttributionEcho {
    fn protocol(&self) -> ProtocolId {
        self.proto.clone()
    }

    async fn handle_inbound(&self, peer: PeerId, mut stream: BoxedStream) -> io::Result<()> {
        let frame = read_frame(&mut stream).await?;
        let marker = String::from_utf8_lossy(&frame).into_owned();
        let _ = self.seen.send((peer, marker));
        write_frame(&mut stream, &frame).await?;
        stream.flush().await
    }

    async fn handle(&self, _stream: BoxedStream) -> io::Result<()> {
        Err(io::Error::other(
            "attributed dispatch must use handle_inbound",
        ))
    }
}

async fn node(tag: &str) -> Node {
    Node::builder()
        .mdns(false)
        .data_dir(std::env::temp_dir().join(format!("itest-attr-{tag}-{}", std::process::id())))
        .build()
        .await
        .expect("node")
}

/// 只登记 QUIC 地址：多流用例统一走 QUIC 原生多流（acp-agent 同款口径）。
fn seed(server: &Node, server_peer: PeerId, client: &Node) {
    for addr in server.listen_addrs() {
        if addr.contains("/u") {
            client.add_peer_address(server_peer, &addr).expect("seed addr");
        }
    }
}

async fn echo_marker(stream: &mut BoxedStream, marker: &str) -> String {
    write_frame(stream, marker.as_bytes()).await.expect("write marker");
    let frame = read_frame(stream).await.expect("echo frame");
    String::from_utf8(frame).expect("utf8 echo")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_streams_attribute_each_to_its_own_peer() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let server = node("hub").await;
    let proto = ProtocolId::new(PROTO).expect("valid protocol id");
    server.handle_protocol(Arc::new(AttributionEcho {
        proto: proto.clone(),
        seen: tx,
    }));
    let server_peer = server.local_peer_id();
    let a = node("peer-a").await;
    let b = node("peer-b").await;
    seed(&server, server_peer, &a);
    seed(&server, server_peer, &b);

    a.connect(server_peer).await.expect("a connect");
    b.connect(server_peer).await.expect("b connect");
    let mut sa = a
        .new_stream(server_peer, proto.clone())
        .await
        .expect("stream from a");
    let mut sb = b
        .new_stream(server_peer, proto.clone())
        .await
        .expect("stream from b");

    let (echo_a, echo_b) =
        tokio::join!(echo_marker(&mut sa, "from-a"), echo_marker(&mut sb, "from-b"));
    assert_eq!(echo_a, "from-a", "echo must come back on its own stream");
    assert_eq!(echo_b, "from-b", "echo must come back on its own stream");

    let seen = tokio::time::timeout(STEP, async {
        vec![
            rx.recv().await.expect("first attribution"),
            rx.recv().await.expect("second attribution"),
        ]
    })
    .await
    .expect("both attributions within step");
    assert!(
        seen.contains(&(a.local_peer_id(), "from-a".to_owned())),
        "stream a must attribute to dialer a: {seen:?}"
    );
    assert!(
        seen.contains(&(b.local_peer_id(), "from-b".to_owned())),
        "stream b must attribute to dialer b: {seen:?}"
    );
    assert_ne!(
        seen[0].0, seen[1].0,
        "two distinct dialers must never collapse into one attribution: {seen:?}"
    );

    server.shutdown();
    a.shutdown();
    b.shutdown();
}
