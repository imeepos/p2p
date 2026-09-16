//! rd 商用收口 itest（M6）：质量协商 / 审批闸 / 断线重连。
//! 质量：viewer quality 请求 → host active_fps 读回断言（避免计时脆弱）。
//! 审批：require_approval 下 hello 拒（awaiting_approval）→ approve → 重连成功。
//! 重连：close 后 host 会话清理，同 peer 重连画面恢复。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use p2p::Node;
use rd_host::{RdHost, SyntheticFactory};
use rd_input::recording::RecordingInjectorFactory;
use rd_viewer::{DecodedFrame, RdViewer, RenderSink};

const STEP: Duration = Duration::from_secs(15);
const W: u16 = 160;
const H: u16 = 90;

async fn build_node(dir: PathBuf) -> Arc<Node> {
    Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(dir)
            .build()
            .await
            .unwrap(),
    )
}

fn tcp_addrs(node: &Node) -> Vec<String> {
    node.listen_addrs()
        .into_iter()
        .filter(|a| a.contains("/t"))
        .collect()
}

struct CollectSink(Arc<Mutex<Vec<DecodedFrame>>>);
impl RenderSink for CollectSink {
    fn on_frame(&self, frame: DecodedFrame) {
        self.0.lock().unwrap().push(frame);
    }
}

async fn wait_frames(frames: &Arc<Mutex<Vec<DecodedFrame>>>, n: usize) {
    let deadline = tokio::time::Instant::now() + STEP;
    loop {
        {
            let g = frames.lock().unwrap();
            if g.len() >= n {
                return;
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timeout waiting {n} frames, got {}",
            frames.lock().unwrap().len()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn wait_session_closed(host: &RdHost) {
    let deadline = tokio::time::Instant::now() + STEP;
    while host.session_count() > 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timeout waiting host session cleanup"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn connect_addr(viewer_node: &Arc<Node>, host_node: &Arc<Node>) -> p2p::PeerId {
    let peer = host_node.local_peer_id();
    let addr = tcp_addrs(host_node).into_iter().next().unwrap();
    viewer_node.add_peer_address(peer, &addr).unwrap();
    peer
}

async fn make_host(tag: &str, require_approval: bool) -> (Arc<Node>, RdHost, PathBuf) {
    let root = std::env::temp_dir().join(format!("rd-m6-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let host_node = build_node(root.join("host")).await;
    let config = rd_host::HostConfig {
        fs_root: root.join("fs"),
        require_approval,
        ..Default::default()
    };
    let host = RdHost::with_config(
        host_node.clone(),
        Arc::new(SyntheticFactory { w: W, h: H }),
        Arc::new(RecordingInjectorFactory::new()),
        Arc::new(rd_clipboard::memory::MemoryClipboardFactory::new()),
        config,
    )
    .unwrap();
    host.set_enabled(true);
    (host_node, host, root)
}

#[tokio::test]
async fn rd_m6_quality_negotiation() {
    let _ = p2p_log::init(Default::default());
    let (host_node, host, root) = make_host("q", false).await;
    let viewer_node = build_node(root.join("viewer")).await;
    let viewer = RdViewer::new(viewer_node.clone());
    let peer = connect_addr(&viewer_node, &host_node);

    let frames = Arc::new(Mutex::new(Vec::new()));
    let session = viewer
        .connect(
            peer,
            "0123456789abcdef".into(),
            Arc::new(CollectSink(frames.clone())),
        )
        .await
        .expect("hello ok");
    wait_frames(&frames, 1).await;
    assert_eq!(host.active_fps(), 15, "默认 15fps");

    session
        .control()
        .quality(2, 100, rd_wire::video::CODEC_RAW_RGBA)
        .await
        .unwrap();
    let deadline = tokio::time::Instant::now() + STEP;
    while host.active_fps() != 2 {
        assert!(tokio::time::Instant::now() < deadline, "quality 未采纳");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(host.active_fps(), 2, "质量协商应生效");

    // 非法档位：host 保持旧档
    session.control().quality(0, 100, 0).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(host.active_fps(), 2, "非法档位不得被采纳");

    session.close().await.expect("close ok");
}

#[tokio::test]
async fn rd_m6_approval_gate() {
    let _ = p2p_log::init(Default::default());
    let (host_node, host, root) = make_host("a", true).await;
    let viewer_node = build_node(root.join("viewer")).await;
    let viewer = RdViewer::new(viewer_node.clone());
    let peer = connect_addr(&viewer_node, &host_node);
    let viewer_peer = viewer_node.local_peer_id();

    // 未批准：hello 被拒（awaiting_approval），登记 pending
    let err = viewer
        .probe(peer, "0123456789abcdef".into())
        .await
        .expect_err("未批准必须被拒");
    assert!(
        err.to_string().contains("awaiting_approval"),
        "拒绝原因: {err}"
    );
    assert!(
        host.pending_approvals().contains(&viewer_peer),
        "viewer 应在待审批集"
    );

    // 审批通过 → 重连成功且画面流出
    assert!(host.approve(&viewer_peer), "approve 应消费 pending");
    let frames = Arc::new(Mutex::new(Vec::new()));
    let session = viewer
        .connect(
            peer,
            "0123456789abcdef".into(),
            Arc::new(CollectSink(frames.clone())),
        )
        .await
        .expect("批准后连接成功");
    wait_frames(&frames, 2).await;
    assert_eq!(host.session_count(), 1);
    session.close().await.expect("close ok");
}

#[tokio::test]
async fn rd_m6_reconnect_after_close() {
    let _ = p2p_log::init(Default::default());
    let (host_node, host, root) = make_host("r", false).await;
    let viewer_node = build_node(root.join("viewer")).await;
    let viewer = RdViewer::new(viewer_node.clone());
    let peer = connect_addr(&viewer_node, &host_node);

    // 第一段会话
    let frames1 = Arc::new(Mutex::new(Vec::new()));
    let s1 = viewer
        .connect(
            peer,
            "0123456789abcdef".into(),
            Arc::new(CollectSink(frames1.clone())),
        )
        .await
        .expect("first connect ok");
    wait_frames(&frames1, 1).await;
    s1.close().await.expect("close ok");
    wait_session_closed(&host).await;
    assert_eq!(host.session_count(), 0);

    // 同 peer 重连：画面恢复
    let frames2 = Arc::new(Mutex::new(Vec::new()));
    let s2 = viewer
        .connect(
            peer,
            "abcdef0123456789".into(),
            Arc::new(CollectSink(frames2.clone())),
        )
        .await
        .expect("reconnect ok");
    wait_frames(&frames2, 2).await;
    assert_eq!(host.session_count(), 1);
    s2.close().await.expect("close ok");
}
