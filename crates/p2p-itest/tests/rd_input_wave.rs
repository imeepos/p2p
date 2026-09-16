//! rd 输入全链 itest（M3）：viewer 沿控制通道发送鼠标/键盘/重置，
//! host 侧 RecordingInjector 依序记录并断言内容与顺序。
//! 真实注入（CGEvent）需辅助功能授权，见 crates/rd-input/src/macos.rs #[ignore] 冒烟。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use p2p::Node;
use rd_host::{RdHost, SyntheticFactory};
use rd_input::recording::RecordingInjectorFactory;
use rd_input::Recorded;
use rd_viewer::RdViewer;

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

/// 等到记录事件达到期望长度并返回快照（有界等待，超时 panic 留信号）。
async fn wait_recorded(events: &Arc<Mutex<Vec<Recorded>>>, want: usize) -> Vec<Recorded> {
    let deadline = tokio::time::Instant::now() + STEP;
    loop {
        {
            let guard = events.lock().unwrap();
            if guard.len() >= want {
                return guard.clone();
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timeout waiting for {want} recorded events, got {}",
            events.lock().unwrap().len()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn rd_input_wave_e2e() {
    let _ = p2p_log::init(Default::default());
    let root = std::env::temp_dir().join(format!("rd-input-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let host_node = build_node(root.join("host")).await;
    let viewer_node = build_node(root.join("viewer")).await;
    let inject_factory = Arc::new(RecordingInjectorFactory::new());
    let recorded = inject_factory.events().clone();
    let host = RdHost::with_config(
        host_node.clone(),
        Arc::new(SyntheticFactory { w: W, h: H }),
        inject_factory,
        Arc::new(rd_clipboard::memory::MemoryClipboardFactory::new()),
        rd_host::HostConfig::default(),
    )
    .unwrap();
    host.set_enabled(true);
    let viewer = RdViewer::new(viewer_node.clone());

    let host_peer = host_node.local_peer_id();
    let host_addr = tcp_addrs(&host_node).into_iter().next().unwrap();
    viewer_node.add_peer_address(host_peer, &host_addr).unwrap();

    // viewer 不接视频也能发输入（control 通道独立）——本测试不发视频流。
    let sink = Arc::new(NoopSink);
    let session = viewer
        .connect(host_peer, SESSION_ID.into(), sink)
        .await
        .expect("hello_ack ok");
    let ctl = session.control();

    // 鼠标序列：按下/松开/移动/滚轮
    ctl.mouse(100, 200, 0b001, 0, 0).await.unwrap();
    ctl.mouse(100, 200, 0b000, 0, 0).await.unwrap();
    ctl.mouse(120, 210, 0b010, 0, -3).await.unwrap(); // 右键按下 + 向上滚 3 格
    ctl.mouse(120, 210, 0b000, 0, 0).await.unwrap();
    // 键盘序列：A 按下/松开，修饰键 Shift 按下后再按 A
    ctl.key(0x04, true, 0).await.unwrap(); // HID A
    ctl.key(0x04, false, 0).await.unwrap();
    ctl.key(0xE1, true, rd_input::mods::SHIFT).await.unwrap(); // LeftShift
    ctl.key(0x04, true, rd_input::mods::SHIFT).await.unwrap();
    ctl.key(0x04, false, 0).await.unwrap();
    // 释放全部按键
    ctl.key_reset().await.unwrap();

    let want = vec![
        Recorded::MouseMove { x: 100, y: 200 },
        Recorded::MouseButton {
            btn: rd_input::MouseButton::Left,
            down: true,
        },
        Recorded::MouseMove { x: 100, y: 200 },
        Recorded::MouseButton {
            btn: rd_input::MouseButton::Left,
            down: false,
        },
        Recorded::MouseMove { x: 120, y: 210 },
        Recorded::MouseButton {
            btn: rd_input::MouseButton::Right,
            down: true,
        },
        Recorded::MouseWheel { dx: 0, dy: -3 },
        Recorded::MouseMove { x: 120, y: 210 },
        Recorded::MouseButton {
            btn: rd_input::MouseButton::Right,
            down: false,
        },
        Recorded::Key {
            code: 0x04,
            down: true,
            modifiers: 0,
        },
        Recorded::Key {
            code: 0x04,
            down: false,
            modifiers: 0,
        },
        Recorded::Key {
            code: 0xE1,
            down: true,
            modifiers: 1,
        },
        Recorded::Key {
            code: 0x04,
            down: true,
            modifiers: 1,
        },
        Recorded::Key {
            code: 0x04,
            down: false,
            modifiers: 0,
        },
        Recorded::Reset,
    ];
    let got = wait_recorded(&recorded, want.len()).await;
    assert_eq!(
        got[..want.len()],
        want[..],
        "注入事件顺序/内容必须与发送一致"
    );
    assert_eq!(host.session_count(), 1, "host 侧恰一个活跃会话");

    session.close().await.expect("close ok");
}

/// 无操作渲染 sink（本测试不发视频流）。
struct NoopSink;
impl rd_viewer::RenderSink for NoopSink {
    fn on_frame(&self, _frame: rd_viewer::DecodedFrame) {}
}
