//! 挂载桥波：VDriveClient(远端) → DavService → 真 TCP 回环 HTTP。
//! 断言 WebDAV 方法面语义与 OS 挂载客户端兼容性关键路径
//! （specs/vdrive.md §5）：PROPFIND/PUT/GET/HEAD/MKCOL/MOVE/COPY/
//! DELETE 递归/404/405/409/越狱 400/大文件流式。

mod vdrive_common;

use std::sync::Arc;

use p2p_vdrive::bridge::{MountBridge, MountConfig};
use p2p_vdrive::FsBackend;
use tokio::sync::watch;
use vdrive_common::{header, http, rig};

struct BridgeRig {
    addr: std::net::SocketAddr,
    client: Arc<p2p_vdrive::VDriveClient>,
    _shutdown: tokio::sync::watch::Sender<bool>,
    host: Arc<p2p::Node>,
    guest: Arc<p2p::Node>,
    /// 宿主根目录生命周期（提前删除会让监狱校验按 InvalidPath 拒绝）。
    _tmp: tempfile::TempDir,
}

async fn bridge_rig(name: &str) -> BridgeRig {
    let rig = rig(name).await;
    let backend: Arc<dyn FsBackend> = rig.client.clone();
    let bridge = MountBridge::bind(MountConfig::default())
        .await
        .unwrap_or_else(|e| panic!("{name}: bind: {e}"));
    let addr = bridge.addr;
    let (tx, rx) = watch::channel(false);
    tokio::spawn(bridge.run(backend, rx));
    BridgeRig {
        addr,
        client: rig.client.clone(),
        _shutdown: tx,
        host: rig.host,
        guest: rig.guest,
        _tmp: rig._tmp,
    }
}

#[tokio::test]
async fn dav_options_and_propfind() {
    let rig = bridge_rig("dav1").await;
    rig.client.mkdir("/docs").await.expect("seed mkdir");
    let seed = rig
        .client
        .create("/docs/hello.txt")
        .await
        .expect("seed file");
    let _ = seed;

    let (status, headers, _) = http(rig.addr, "OPTIONS", "/", &[], b"").await;
    assert_eq!(status, 200);
    assert_eq!(header(&headers, "DAV"), Some("1, 2"), "Class 1+2 声明");

    let (status, _, body) = http(rig.addr, "PROPFIND", "/", &[("Depth", "1")], b"").await;
    assert_eq!(status, 207, "PROPFIND 根");
    let xml = String::from_utf8_lossy(&body);
    assert!(xml.contains("<D:href>/</D:href>"), "{xml}");
    assert!(xml.contains("<D:href>/docs/</D:href>"), "{xml}");

    let (status, _, body) = http(rig.addr, "PROPFIND", "/docs/", &[("Depth", "1")], b"").await;
    assert_eq!(status, 207);
    assert!(String::from_utf8_lossy(&body).contains("hello.txt"));

    let (status, _, _) = http(rig.addr, "PROPFIND", "/missing", &[("Depth", "1")], b"").await;
    assert_eq!(status, 404);

    // Depth: infinity（含缺省 Depth，RFC 默认即 infinity）显式 400。
    let (status, _, _) = http(rig.addr, "PROPFIND", "/", &[("Depth", "infinity")], b"").await;
    assert_eq!(status, 400);
    let (status, _, _) = http(rig.addr, "PROPFIND", "/", &[], b"").await;
    assert_eq!(status, 400, "缺 Depth = infinity 默认");
    rig.host.shutdown();
    rig.guest.shutdown();
}

#[tokio::test]
async fn dav_put_get_large_file_streaming() {
    let rig = bridge_rig("dav2").await;
    let mut payload = vec![0u8; MAX * 2 + 333];
    for (i, b) in payload.iter_mut().enumerate() {
        *b = (i % 249) as u8;
    }
    let (status, _, _) = http(rig.addr, "PUT", "/big%20file.bin", &[], &payload).await;
    assert_eq!(status, 201, "PUT 新建");

    // PUT 走 chunked 传输编码（上行流式解码路径）：1000 字节单 chunk
    // 帧形态：size CRLF data CRLF 终块 0 CRLF CRLF
    let mut chunked = format!("{:x}\r\n", 1000).into_bytes();
    chunked.extend_from_slice(&payload[..1000]);
    chunked.extend_from_slice(b"\r\n0\r\n\r\n");
    let (status, _, body) = http(
        rig.addr,
        "PUT",
        "/chunked.txt",
        &[("Transfer-Encoding", "chunked")],
        &chunked,
    )
    .await;
    assert_eq!(
        status,
        201,
        "chunked PUT 失败: {}",
        String::from_utf8_lossy(&body)
    );

    let (status, headers, body) = http(rig.addr, "GET", "/big%20file.bin", &[], b"").await;
    assert_eq!(status, 200);
    assert_eq!(body, payload, "大文件流式下行逐字节一致");
    assert_eq!(
        header(&headers, "Content-Length"),
        Some(payload.len().to_string().as_str())
    );

    let (status, headers, _) = http(rig.addr, "HEAD", "/big%20file.bin", &[], b"").await;
    assert_eq!(status, 200);
    assert_eq!(
        header(&headers, "Content-Length"),
        Some(payload.len().to_string().as_str()),
        "HEAD 报定长"
    );

    let (status, _, body) = http(rig.addr, "GET", "/chunked.txt", &[], b"").await;
    assert_eq!(status, 200);
    assert_eq!(body.len(), 1000, "chunked 上行落盘长度");

    let (status, _, _) = http(rig.addr, "GET", "/nope.bin", &[], b"").await;
    assert_eq!(status, 404);
    rig.host.shutdown();
    rig.guest.shutdown();
}

#[tokio::test]
async fn dav_mkcol_move_copy_delete() {
    let rig = bridge_rig("dav3").await;
    let (status, _, _) = http(rig.addr, "MKCOL", "/dir", &[], b"").await;
    assert_eq!(status, 201);
    let (status, _, _) = http(rig.addr, "MKCOL", "/dir", &[], b"").await;
    assert_eq!(status, 405, "已存在");
    let (status, _, _) = http(rig.addr, "MKCOL", "/no/parent/x", &[], b"").await;
    assert_eq!(status, 409, "父目录缺失");

    let (status, _, _) = http(rig.addr, "PUT", "/dir/a.txt", &[], b"AAA").await;
    assert_eq!(status, 201);
    let (status, _, _) = http(rig.addr, "PUT", "/dir/a.txt", &[], b"BBBB").await;
    assert_eq!(status, 204, "PUT 覆盖");

    let (status, _, _) = http(
        rig.addr,
        "MOVE",
        "/dir/a.txt",
        &[("Destination", "http://bridge/dir/b.txt")],
        b"",
    )
    .await;
    assert_eq!(status, 201);
    let (_, _, body) = http(rig.addr, "GET", "/dir/b.txt", &[], b"").await;
    assert_eq!(body, b"BBBB", "MOVE 后内容不变");

    let (status, _, _) = http(
        rig.addr,
        "COPY",
        "/dir/b.txt",
        &[("Destination", "/dir/c.txt")],
        b"",
    )
    .await;
    assert_eq!(status, 201);
    let (status, _, _) = http(
        rig.addr,
        "COPY",
        "/dir/b.txt",
        &[("Destination", "/dir/c.txt"), ("Overwrite", "F")],
        b"",
    )
    .await;
    assert_eq!(status, 412, "拒绝覆盖");

    let (status, _, _) = http(rig.addr, "DELETE", "/dir", &[], b"").await;
    assert_eq!(status, 204, "递归删除非空目录");
    let (status, _, _) = http(rig.addr, "PROPFIND", "/dir", &[("Depth", "1")], b"").await;
    assert_eq!(status, 404, "整树消失");

    let (status, _, _) = http(rig.addr, "DELETE", "/", &[], b"").await;
    assert_eq!(status, 403, "根保护");

    let (status, _, _) = http(rig.addr, "PUT", "/../escape", &[], b"x").await;
    assert_eq!(status, 400, "越狱路径拒绝");

    let (status, _, _) = http(rig.addr, "BREW", "/", &[], b"").await;
    assert_eq!(status, 405, "未知方法");
    rig.host.shutdown();
    rig.guest.shutdown();
}

const MAX: usize = 512 * 1024;
