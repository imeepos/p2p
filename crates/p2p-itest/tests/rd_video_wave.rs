//! rd 视频全链 itest（M2）：A=host（SyntheticFactory）B=viewer，真实双 Node。
//! 验收：hello_ack 接受 / 首帧 keyframe / 帧内容与合成图案一致 / seq 递增 /
//!       显式 close 后 host 会话清理 / 同 peer 重复拨号被拒。
//! 端口纪律：NodeBuilder 默认端口 0（随机）+ mDNS 关闭，并行测试不撞口。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use p2p::Node;
use rd_capture::synthetic::SyntheticSource;
use rd_host::{RdHost, SyntheticFactory};
use rd_viewer::{DecodedFrame, RdViewer, RenderSink};

const STEP: Duration = Duration::from_secs(15);
const W: u16 = 160;
const H: u16 = 90;
const SESSION_ID: &str = "0123456789abcdef";

/// 帧收集 sink（测试断言用）。
struct CollectSink(Arc<Mutex<Vec<DecodedFrame>>>);

impl RenderSink for CollectSink {
    fn on_frame(&self, frame: DecodedFrame) {
        self.0.lock().unwrap().push(frame);
    }
}

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

/// 等 sink 收集到 n 帧（有界等待，超时 panic 留信号）。
async fn wait_frames(sink: &Arc<Mutex<Vec<DecodedFrame>>>, n: usize) -> Vec<DecodedFrame> {
    let deadline = tokio::time::Instant::now() + STEP;
    loop {
        {
            let guard = sink.lock().unwrap();
            if guard.len() >= n {
                return guard.clone();
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timeout waiting for {n} frames, got {}",
            sink.lock().unwrap().len()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn wait_session_closed(host: &RdHost) {
    let deadline = tokio::time::Instant::now() + STEP;
    while host.session_count() > 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timeout waiting for host session cleanup"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn rd_video_wave_e2e() {
    let _ = p2p_log::init(Default::default());
    let root = std::env::temp_dir().join(format!("rd-video-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let host_node = build_node(root.join("host")).await;
    let viewer_node = build_node(root.join("viewer")).await;
    let host = RdHost::new(host_node.clone(), Arc::new(SyntheticFactory { w: W, h: H })).unwrap();
    let viewer = RdViewer::new(viewer_node.clone());

    let host_peer = host_node.local_peer_id();
    let host_addr = tcp_addrs(&host_node).into_iter().next().unwrap();
    viewer_node.add_peer_address(host_peer, &host_addr).unwrap();

    let frames = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::new(CollectSink(frames.clone()));
    let session = viewer
        .connect(host_peer, SESSION_ID.into(), sink)
        .await
        .expect("hello_ack ok");

    let got = wait_frames(&frames, 3).await;
    assert!(got[0].keyframe, "首帧必须是 keyframe（同步点）");
    for f in &got {
        assert_eq!((f.w, f.h), (W, H), "分辨率必须与源一致");
        assert!(
            SyntheticSource::expect_frame(f.seq, W, H, &f.rgba),
            "帧内容必须与合成图案一致 (seq {})",
            f.seq
        );
    }
    let seqs: Vec<u32> = got.iter().map(|f| f.seq).collect();
    assert!(
        seqs.windows(2).all(|p| p[0] < p[1]),
        "seq 必须严格递增: {seqs:?}"
    );
    assert_eq!(host.session_count(), 1, "host 侧恰一个活跃会话");

    session.close().await.expect("close ok");
    wait_session_closed(&host).await;
}

#[tokio::test]
async fn rd_video_duplicate_connect_rejected() {
    let _ = p2p_log::init(Default::default());
    let root = std::env::temp_dir().join(format!("rd-video-reject-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let host_node = build_node(root.join("host")).await;
    let viewer_node = build_node(root.join("viewer")).await;
    let _host = RdHost::new(host_node.clone(), Arc::new(SyntheticFactory { w: W, h: H })).unwrap();
    let viewer = RdViewer::new(viewer_node.clone());

    let host_peer = host_node.local_peer_id();
    let host_addr = tcp_addrs(&host_node).into_iter().next().unwrap();
    viewer_node.add_peer_address(host_peer, &host_addr).unwrap();

    let frames = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::new(CollectSink(frames.clone()));
    let session = viewer
        .connect(host_peer, SESSION_ID.into(), sink.clone())
        .await
        .expect("first connect ok");

    // 同 peer 二拨：host 侧同 Peer 单活跃会话 → hello_ack{ok:false}
    let err = viewer.connect(host_peer, SESSION_ID.into(), sink).await;
    assert!(err.is_err(), "重复拨号必须被拒");
    session.close().await.expect("close ok");
}
/// zlib 编码全链：host 压缩 → viewer inflate → 内容仍与合成图案一致。
#[tokio::test]
async fn rd_video_zlib_codec_roundtrip() {
    let _ = p2p_log::init(Default::default());
    let root = std::env::temp_dir().join(format!("rd-video-zlib-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let host_node = build_node(root.join("host")).await;
    let viewer_node = build_node(root.join("viewer")).await;
    let config = rd_host::HostConfig {
        codec: rd_wire::video::CODEC_ZLIB_RGBA,
        ..Default::default()
    };
    let _host = RdHost::with_config(
        host_node.clone(),
        Arc::new(SyntheticFactory { w: W, h: H }),
        Arc::new(rd_input::recording::RecordingInjectorFactory::new()),
        Arc::new(rd_clipboard::memory::MemoryClipboardFactory::new()),
        config,
    )
    .unwrap();
    let viewer = RdViewer::new(viewer_node.clone());

    let host_peer = host_node.local_peer_id();
    let host_addr = tcp_addrs(&host_node).into_iter().next().unwrap();
    viewer_node.add_peer_address(host_peer, &host_addr).unwrap();

    let frames = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::new(CollectSink(frames.clone()));
    let session = viewer
        .connect(host_peer, SESSION_ID.into(), sink)
        .await
        .expect("hello_ack ok");
    let got = wait_frames(&frames, 3).await;
    for f in &got {
        assert_eq!((f.w, f.h), (W, H));
        assert!(
            SyntheticSource::expect_frame(f.seq, W, H, &f.rgba),
            "zlib 解压后内容必须一致 (seq {})",
            f.seq
        );
    }
    session.close().await.expect("close ok");
}
