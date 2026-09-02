//! 观测回归（coordination.md S 包补充项，design §7.2）：
//! bootstrap 反射器学习对端公网映射地址 → 节点把观测地址与监听地址一并
//! 注册进 rendezvous → 其他节点经发现拿到可拨地址并直连成功。

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
    let dir = std::env::temp_dir().join(format!("p2p-obs-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    dir
}

async fn expect_event(
    rx: &mut broadcast::Receiver<NodeEvent>,
    want: &dyn Fn(&NodeEvent) -> bool,
    what: &str,
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

/// 观测 → 注册 → 发现 → 直连：A 的观测地址经 rendezvous 到达 B，直连 roundtrip。
#[tokio::test]
async fn observed_addr_registered_and_dialable() {
    let r_dir = tmp_dir("obs-r");
    let a_dir = tmp_dir("obs-a");
    let b_dir = tmp_dir("obs-b");

    // bootstrap 角色：rendezvous 服务（facade 节点自带）+ 观测反射器
    let node_r = Node::builder()
        .mdns(false)
        .data_dir(r_dir.clone())
        .observation_responder(0)
        .build()
        .await
        .expect("node r");
    let r_obs = node_r
        .observation_addr()
        .expect("reflector addr must be exposed");
    let r_bootstrap = node_r
        .listen_addrs()
        .into_iter()
        .find(|a| a.contains("/u"))
        .expect("bootstrap quic addr");

    // A：向 bootstrap 观测口学习外部地址，并注册进 rendezvous
    let node_a = Node::builder()
        .mdns(false)
        .data_dir(a_dir.clone())
        .bootstrap(vec![r_bootstrap.clone()])
        .observation_addrs(vec![format!("127.0.0.1:{}", r_obs.port())])
        .build()
        .await
        .expect("node a");
    let a_peer = node_a.local_peer_id();
    let a_quic = node_a
        .listen_addrs()
        .into_iter()
        .find(|a| a.contains("/u"))
        .expect("a quic addr");

    node_a.handle_protocol(Arc::new(Echo));

    // B：经 rendezvous 发现 A，直连 echo
    let node_b = Node::builder()
        .mdns(false)
        .data_dir(b_dir.clone())
        .bootstrap(vec![r_bootstrap])
        .build()
        .await
        .expect("node b");
    let mut events_b = node_b.events();

    let discovered = expect_event(
        &mut events_b,
        &|ev| matches!(ev, NodeEvent::PeerDiscovered { peer, .. } if *peer == a_peer),
        "rendezvous PeerDiscovered",
    )
    .await;
    // 注册地址含观测合成项（观测 IP + A 的 QUIC 端口）
    assert!(
        matches!(&discovered, NodeEvent::PeerDiscovered { addrs, .. }
            if addrs.iter().any(|s| s == &a_quic)),
        "registered addrs must include observed-composed quic addr"
    );

    node_b
        .connect(a_peer)
        .await
        .expect("direct dial via observed book");
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
            b"observed".to_vec(),
            Duration::from_secs(10),
        )
        .await
        .expect("echo roundtrip");
    assert_eq!(reply, b"observed");

    node_r.shutdown();
    node_a.shutdown();
    node_b.shutdown();
    let _ = std::fs::remove_dir_all(&r_dir);
    let _ = std::fs::remove_dir_all(&a_dir);
    let _ = std::fs::remove_dir_all(&b_dir);
}
