//! rd 剪贴板全链 itest（M4）：viewer→host 推送、host 外部变更→viewer 下行、
//! 回声抑制（viewer 推送不被 host 弹回）。
//! 两侧均用共享内存后端（MemoryClipboardFactory），零系统剪贴板依赖。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use p2p::Node;
use rd_clipboard::memory::MemoryClipboardFactory;
use rd_host::{RdHost, SyntheticFactory};
use rd_input::recording::RecordingInjectorFactory;
use rd_viewer::{RdViewer, RenderSink};

const STEP: Duration = Duration::from_secs(15);
const W: u16 = 160;
const H: u16 = 90;
const SESSION_ID: &str = "0123456789abcdef";

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

async fn wait_clip(clip: &Arc<Mutex<Option<String>>>, want: &str) {
    let deadline = tokio::time::Instant::now() + STEP;
    loop {
        {
            let g = clip.lock().unwrap();
            if g.as_deref() == Some(want) {
                return;
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timeout waiting clipboard == {want:?}, got {:?}",
            clip.lock().unwrap().as_deref()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 无操作渲染 sink（本测试不发视频流）。
struct NoopSink;
impl RenderSink for NoopSink {
    fn on_frame(&self, _frame: rd_viewer::DecodedFrame) {}
}

#[tokio::test]
async fn rd_clipboard_wave_e2e() {
    let _ = p2p_log::init(Default::default());
    let root = std::env::temp_dir().join(format!("rd-clip-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let host_node = build_node(root.join("host")).await;
    let viewer_node = build_node(root.join("viewer")).await;

    let host_clip_factory = Arc::new(MemoryClipboardFactory::new());
    let host_clip = host_clip_factory.inner().clone();
    let host = RdHost::with_config(
        host_node.clone(),
        Arc::new(SyntheticFactory { w: W, h: H }),
        Arc::new(RecordingInjectorFactory::new()),
        host_clip_factory.clone(),
        rd_host::HostConfig::default(),
    )
    .unwrap();
    let viewer = RdViewer::new(viewer_node.clone());

    let host_peer = host_node.local_peer_id();
    let host_addr = tcp_addrs(&host_node).into_iter().next().unwrap();
    viewer_node.add_peer_address(host_peer, &host_addr).unwrap();

    let viewer_clip: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let viewer_backend = {
        let inner = viewer_clip.clone();
        rd_clipboard::memory::MemoryClipboard::new(inner)
    };
    let session = viewer
        .connect_full(
            host_peer,
            SESSION_ID.into(),
            Arc::new(NoopSink),
            Some(Arc::new(tokio::sync::Mutex::new(viewer_backend))),
        )
        .await
        .expect("hello_ack ok");
    let ctl = session.control();

    // 场景 A：viewer → host 推送，host 写本机；且 host 不弹回（回声抑制）
    ctl.clipboard("hello from viewer".into()).await.unwrap();
    wait_clip(&host_clip, "hello from viewer").await;
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(
        viewer_clip.lock().unwrap().as_deref(),
        None,
        "回声抑制失败：host 不应把 viewer 推送弹回"
    );

    // 场景 B：host 外部变更 → 下行到 viewer 本机
    let host_factory_clone = host_clip_factory.clone();
    host_factory_clone.set_external("from host clipboard");
    wait_clip(&viewer_clip, "from host clipboard").await;

    // 场景 C：host 再变更 → viewer 再收到（轮询持续生效）
    host_factory_clone.set_external("second change");
    wait_clip(&viewer_clip, "second change").await;

    assert_eq!(host.session_count(), 1);
    session.close().await.expect("close ok");
}
