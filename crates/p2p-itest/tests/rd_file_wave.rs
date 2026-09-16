//! rd 文件全链 itest（M5）：/rd/file/1 浏览/建删/上传/下载 + 路径卫生。
//! host 隔离根 = 临时目录；2 MiB 随机文件跨 chunk 边界（384 KiB base64 块）。

use std::path::PathBuf;
use std::sync::Arc;

use p2p::Node;
use rd_host::{RdHost, SyntheticFactory};
use rd_input::recording::RecordingInjectorFactory;
use rd_viewer::{RdViewer, RenderSink};

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

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

struct NoopSink;
impl RenderSink for NoopSink {
    fn on_frame(&self, _frame: rd_viewer::DecodedFrame) {}
}

#[tokio::test]
async fn rd_file_wave_e2e() {
    let _ = p2p_log::init(Default::default());
    let root = std::env::temp_dir().join(format!("rd-file-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let host_root = root.join("host-files");
    std::fs::create_dir_all(&host_root).unwrap();
    std::fs::write(host_root.join("seed.txt"), "seed-content-0123456789").unwrap();

    let host_node = build_node(root.join("node-host")).await;
    let viewer_node = build_node(root.join("node-viewer")).await;
    let config = rd_host::HostConfig {
        fs_root: host_root.clone(),
        ..Default::default()
    };
    let _host = RdHost::with_config(
        host_node.clone(),
        Arc::new(SyntheticFactory { w: W, h: H }),
        Arc::new(RecordingInjectorFactory::new()),
        Arc::new(rd_clipboard::memory::MemoryClipboardFactory::new()),
        config,
    )
    .unwrap();
    _host.set_enabled(true);
    let viewer = RdViewer::new(viewer_node.clone());

    let host_peer = host_node.local_peer_id();
    let host_addr = tcp_addrs(&host_node).into_iter().next().unwrap();
    viewer_node.add_peer_address(host_peer, &host_addr).unwrap();
    let session = viewer
        .connect(host_peer, SESSION_ID.into(), Arc::new(NoopSink))
        .await
        .expect("hello_ack ok");

    // 浏览根：seed.txt 在列
    let entries = session.fs().list("").await.expect("list root");
    assert!(
        entries.iter().any(|e| e.name == "seed.txt"),
        "seed 文件可见"
    );

    // 建目录 + 删除
    session.fs().mkdir("subdir/nested").await.expect("mkdir ok");
    let entries = session.fs().list("subdir").await.expect("list subdir");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "nested");
    session.fs().rm("subdir").await.expect("rm ok");
    assert!(!host_root.join("subdir").exists());

    // 上传 2 MiB 随机文件（跨多个 384 KiB 块）
    let local_up = root.join("upload.bin");
    let payload: Vec<u8> = (0..2 * 1024 * 1024)
        .map(|i| ((i as u64).wrapping_mul(31) % 251) as u8)
        .collect();
    std::fs::write(&local_up, &payload).unwrap();
    let mut prog: Vec<(u64, u64)> = Vec::new();
    session
        .fs()
        .upload(&local_up, "uploads/blob.bin", |done, total| {
            prog.push((done, total))
        })
        .await
        .expect("upload ok");
    let host_blob = std::fs::read(host_root.join("uploads/blob.bin")).unwrap();
    eprintln!(
        "DEBUG: host_blob.len={} payload.len={}",
        host_blob.len(),
        payload.len()
    );
    eprintln!(
        "DEBUG: host head={:?} pay head={:?}",
        &host_blob[..8],
        &payload[..8]
    );
    eprintln!(
        "DEBUG: host tail={:?} pay tail={:?}",
        &host_blob[host_blob.len() - 8..],
        &payload[payload.len() - 8..]
    );
    assert_eq!(
        sha256_hex(&host_blob),
        sha256_hex(&payload),
        "上传内容必须一致"
    );
    assert_eq!(
        prog.last().map(|(d, t)| (*d, *t)),
        Some((2 * 1024 * 1024, 2 * 1024 * 1024)),
        "进度必须收敛到完成"
    );

    // 下载 seed.txt 到本地
    let local_dl = root.join("download.txt");
    session
        .fs()
        .download("seed.txt", &local_dl, |_, _| {})
        .await
        .expect("download ok");
    assert_eq!(
        std::fs::read_to_string(&local_dl).unwrap(),
        "seed-content-0123456789",
        "下载内容必须一致"
    );

    // stat
    let st = session
        .fs()
        .stat("uploads/blob.bin")
        .await
        .expect("stat ok");
    assert_eq!(st.unwrap().size, payload.len() as u64);

    // 路径逃逸拒绝（wire 层卫生；host 侧 FsService 双保险）
    assert!(session.fs().list("../").await.is_err(), ".. 必须拒绝");
    assert!(session.fs().list("/etc").await.is_err(), "绝对路径必须拒绝");
    assert!(session.fs().mkdir("a/../../b").await.is_err());

    session.close().await.expect("close ok");
}
