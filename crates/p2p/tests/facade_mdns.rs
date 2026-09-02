//! mdns 自动发现变体（coordination.md S 包）：本机组播环境不稳定时跳过，
//! 手动运行：cargo test -p p2p --test facade_mdns -- --ignored
//! 每条连接只做一次开流：yamux 驱动在空闲连接上的第二次 open_stream 存在
//! 唤醒丢失缺陷（p2p-mux 上游问题，已报告），多流场景待上游修复后补验收。

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use p2p::{Node, NodeEvent, ProtocolHandler, ProtocolId};
use p2p_mux::BoxedStream;
use p2p_protocol::{read_frame, write_frame};
use tokio::sync::broadcast;

const ECHO_PROTOCOL: &str = "/test/echo/1";
const WAIT: Duration = Duration::from_secs(30);

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

#[tokio::test]
#[ignore = "mdns 组播依赖本机网络环境，不稳定；需要时手动运行"]
async fn mdns_discovered_roundtrip() {
    let a_dir = tmp_dir("mdns-a");
    let b_dir = tmp_dir("mdns-b");
    let node_a = Node::builder()
        .mdns(true)
        .data_dir(a_dir.clone())
        .build()
        .await
        .expect("node a");
    let node_b = Node::builder()
        .mdns(true)
        .data_dir(b_dir.clone())
        .build()
        .await
        .expect("node b");
    let a_peer = node_a.local_peer_id();

    node_a.handle_protocol(Arc::new(Echo));
    let mut events_b = node_b.events();

    expect_event(
        &mut events_b,
        &|ev| matches!(ev, NodeEvent::PeerDiscovered { peer, .. } if *peer == a_peer),
        "mdns PeerDiscovered",
    )
    .await;

    node_b.connect(a_peer).await.expect("connect after mdns discovery");
    let reply = node_b
        .request(
            a_peer,
            ProtocolId::new(ECHO_PROTOCOL).expect("id"),
            b"mdns".to_vec(),
            Duration::from_secs(5),
        )
        .await
        .expect("mdns roundtrip");
    assert_eq!(reply, b"mdns");

    node_a.shutdown();
    node_b.shutdown();
    let _ = std::fs::remove_dir_all(&a_dir);
    let _ = std::fs::remove_dir_all(&b_dir);
}
